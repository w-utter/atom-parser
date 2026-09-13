use crate::{IoError, SwapOffsets};
use core::task::{Poll, Context};
use core::pin::Pin;
use core::marker::PhantomPinned;
use pin_project::pin_project;

/// the backing reader for an async reader
/// e.g anything that implements pollreader
/// also implements asyncreader
pub trait PollReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(), IoError>>;

    fn poll_read_cstr(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<usize, IoError>>;

    fn seek_start(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        amt: usize,
    ) -> Result<(), IoError>;

    fn poll_seek_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), IoError>>;

    fn offset(&self) -> usize;
    fn remaining_size(&self) -> usize;
}

impl <'a, R: ?Sized + Unpin + PollReader> PollReader for &'a mut R {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(), IoError>> {
        Pin::new(&mut **self).poll_read(cx, buf)
    }

    fn poll_read_cstr(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<usize, IoError>> {
        Pin::new(&mut **self).poll_read_cstr(cx)
    }

    fn seek_start(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        amt: usize,
    ) -> Result<(), IoError> {
        Pin::new(&mut **self).seek_start(cx, amt)
    }

    fn poll_seek_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), IoError>> {
        Pin::new(&mut **self).poll_seek_complete(cx)
    }

    fn offset(&self) -> usize {
        R::offset(self)
    }

    fn remaining_size(&self) -> usize {
        R::remaining_size(self)
    }
}

pub trait AsyncReader: SwapOffsets + PollReader + Unpin {
    fn async_read<'a>(&'a mut self, bytes: &'a mut [u8]) -> AsyncRead<'a, &'a Self> {
        AsyncRead::read(self, bytes)
    }
    fn async_seek(&mut self, amt: usize) -> AsyncSeek<&mut Self> {
        AsyncSeek::seek(self, amt)
    }

    // reads a cstr and returns its length (including nul)
    fn async_read_cstr(&mut self) -> AsyncReadCstr<&mut Self> {
        AsyncReadCstr::read_cstr(self)
    }

    fn seek_remaining(&mut self) -> AsyncSeek<&mut Self> {
        self.async_seek(self.remaining_size())
    }
}

impl <R: PollReader + SwapOffsets + Unpin> AsyncReader for R {}


#[pin_project]
pub struct AsyncRead<'a, R> {
    reader: R,
    buf: &'a mut [u8],
    #[pin]
    _pin: PhantomPinned,
}

impl <'a, R> AsyncRead<'a, R> {
    pub fn read(reader: R, buf: &'a mut [u8]) -> Self {
        Self {
            reader,
            buf,
            _pin: PhantomPinned,
        }
    }
}

#[pin_project(UnsafeUnpin)]
pub struct AsyncReadCstr<R> {
    pub reader: R,
    #[pin]
    _pin: PhantomPinned,
}

impl <R: PollReader> AsyncReadCstr<R> {
    pub fn read_cstr(reader: R) -> Self {
        Self {
            reader,
            _pin: PhantomPinned,
        }
    }
}

unsafe impl <R: Unpin> pin_project::UnsafeUnpin for AsyncReadCstr<R> {}

impl <R: PollReader + Unpin> Future for AsyncReadCstr<R> {
    type Output = Result<usize, IoError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<usize, IoError>> {
        let me = self.project();
        let len = core::task::ready!(Pin::new(me.reader).poll_read_cstr(cx))?;
        Poll::Ready(Ok(len))
    }
}

#[pin_project(UnsafeUnpin)]
pub struct AsyncSeek<R> {
    pub reader: R,
    amt: Option<usize>,
    #[pin]
    _pin: PhantomPinned,
}

impl <R: PollReader> AsyncSeek<R> {
    pub fn seek(reader: R, amt: usize) -> AsyncSeek<R> {
        Self {
            reader,
            amt: Some(amt),
            _pin: PhantomPinned,
        }
    }
}

unsafe impl <R: Unpin> pin_project::UnsafeUnpin for AsyncSeek<R> {}

impl <R: PollReader + Unpin> Future for AsyncSeek<R> {
    type Output = Result<(), IoError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
        let me = self.project();
        match me.amt {
            Some(amt) => {
                core::task::ready!(Pin::new(&mut *me.reader).poll_seek_complete(cx))?;
                match Pin::new(&mut *me.reader).seek_start(cx, *amt) {
                    Ok(()) => {
                        *me.amt = None;
                        Pin::new(&mut *me.reader).poll_seek_complete(cx)
                    }
                    Err(e) => Poll::Ready(Err(e))
                }
            }
            None => {
                Pin::new(&mut *me.reader).poll_seek_complete(cx)
            }
        }
    }
}
