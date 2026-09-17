use crate::{IoError, SwapOffsets, Offset};
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project::pin_project;

/// the backing reader for an async reader
/// e.g anything that implements pollreader
/// also implements asyncreader
pub trait PollReader<O: Offset> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(), IoError>>;

    fn poll_read_cstr(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<O, IoError>>;

    fn seek_start(self: Pin<&mut Self>, cx: &mut Context<'_>, amt: O) -> Result<(), IoError>;

    fn poll_seek_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), IoError>>;

    fn offset(&self) -> O;
    fn remaining_size(&self) -> O;
}

impl<'a, O: Offset, R: Unpin + PollReader<O>> PollReader<O> for &'a mut R {
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
    ) -> Poll<Result<O, IoError>> {
        Pin::new(&mut **self).poll_read_cstr(cx)
    }

    fn seek_start(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        amt: O,
    ) -> Result<(), IoError> {
        Pin::new(&mut **self).seek_start(cx, amt)
    }

    fn poll_seek_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), IoError>> {
        Pin::new(&mut **self).poll_seek_complete(cx)
    }

    fn offset(&self) -> O {
        R::offset(self)
    }

    fn remaining_size(&self) -> O {
        R::remaining_size(self)
    }
}

pub trait AsyncReader<O: Offset>: SwapOffsets<O> + PollReader<O> + Unpin + Sized {
    fn async_read<'a>(&'a mut self, bytes: &'a mut [u8]) -> AsyncRead<'a, &'a Self> {
        AsyncRead::read(self, bytes)
    }
    fn async_seek(&mut self, amt: O) -> AsyncSeek<O, &mut Self> {
        AsyncSeek::seek(self, amt)
    }

    // reads a cstr and returns its length (including nul)
    fn async_read_cstr(&mut self) -> AsyncReadCstr<O, &mut Self> {
        AsyncReadCstr::read_cstr(self)
    }

    fn seek_remaining(&mut self) -> AsyncSeek<O, &mut Self> {
        self.async_seek(self.remaining_size())
    }
}

impl<O: Offset, R: PollReader<O> + SwapOffsets<O> + Unpin> AsyncReader<O> for R {}

#[pin_project]
pub struct AsyncRead<'a, R> {
    reader: R,
    buf: &'a mut [u8],
    #[pin]
    _pin: PhantomPinned,
}

impl<'a, R> AsyncRead<'a, R> {
    pub fn read(reader: R, buf: &'a mut [u8]) -> Self {
        Self {
            reader,
            buf,
            _pin: PhantomPinned,
        }
    }
}

#[pin_project(UnsafeUnpin)]
pub struct AsyncReadCstr<O, R> {
    pub reader: R,
    #[pin]
    _pin: PhantomPinned,
    _pd: core::marker::PhantomData<O>,
}

impl<O: Offset, R: PollReader<O>> AsyncReadCstr<O, R> {
    pub fn read_cstr(reader: R) -> Self {
        Self {
            reader,
            _pin: PhantomPinned,
            _pd: core::marker::PhantomData,
        }
    }
}

unsafe impl<O: Offset + Unpin, R: PollReader<O> +Unpin> pin_project::UnsafeUnpin for AsyncReadCstr<O, R> {}

impl<O: Offset, R: PollReader<O> + Unpin> Future for AsyncReadCstr<O, R> {
    type Output = Result<O, IoError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<O, IoError>> {
        let me = self.project();
        let len = core::task::ready!(Pin::new(me.reader).poll_read_cstr(cx))?;
        Poll::Ready(Ok(len))
    }
}

#[pin_project(UnsafeUnpin)]
pub struct AsyncSeek<O, R> {
    pub reader: R,
    amt: Option<O>,
    #[pin]
    _pin: PhantomPinned,
}

impl<O: Offset, R: PollReader<O>> AsyncSeek<O, R> {
    pub fn seek(reader: R, amt: O) -> AsyncSeek<O, R> {
        Self {
            reader,
            amt: Some(amt),
            _pin: PhantomPinned,
        }
    }
}

unsafe impl<O: Unpin, R: Unpin> pin_project::UnsafeUnpin for AsyncSeek<O, R> {}

impl<O: Offset, R: PollReader<O> + Unpin> Future for AsyncSeek<O, R> {
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
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            None => Pin::new(&mut *me.reader).poll_seek_complete(cx),
        }
    }
}
