use crate::{IoError, Offset, SwapOffsets};
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

    fn extent(&self) -> crate::Extent<O, O> {
        let offset = self.offset();
        let len = self.remaining_size();
        crate::Extent { offset, len }
    }

    #[cfg(feature = "alloc")]
    fn poll_read_extent<A: alloc::alloc::Allocator>(
        self: Pin<&mut Self>,
        extent: &crate::Extent<O, O>,
        buf: &mut alloc::vec::Vec<u8, A>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), crate::ExtentError>>;

    fn poll_read_extent_with(
        self: Pin<&mut Self>,
        extent: &crate::Extent<O, O>,
        buf: &mut [u8],
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), crate::IoError>>;
}

#[cfg(feature = "alloc")]
pub struct ReadingExtent<O: Offset, R: PollReader<O>, A: alloc::alloc::Allocator> {
    pub reader: R,
    pub extent: crate::Extent<O, O>,
    pub buffer: alloc::vec::Vec<u8, A>,
}

#[cfg(feature = "alloc")]
impl<O: Offset + Unpin, R: PollReader<O> + Unpin, A: alloc::alloc::Allocator + Unpin> Future
    for ReadingExtent<O, R, A>
{
    type Output = Result<(), crate::ExtentError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            extent,
            buffer,
        } = &mut *self;
        Pin::new(reader).poll_read_extent(extent, buffer, cx)
    }
}

#[cfg(feature = "alloc")]
pub enum AsyncReadExtent<
    O: Offset,
    R: PollReader<O>,
    A: alloc::alloc::Allocator,
    S1: TryInto<O>,
    S2: TryInto<O>,
> {
    Setup {
        extent: crate::Extent<S1, S2>,
        alloc: A,
        reader: R,
    },
    Reading(ReadingExtent<O, R, A>),
    Done,
}

#[cfg(feature = "alloc")]
impl<
    O: Offset + Unpin,
    R: PollReader<O> + Unpin,
    A: alloc::alloc::Allocator + Unpin,
    S1: TryInto<O> + Unpin,
    S2: TryInto<O> + TryInto<usize> + Copy + Unpin,
> Future for AsyncReadExtent<O, R, A, S1, S2>
{
    type Output = Result<alloc::boxed::Box<[u8], A>, crate::ExtentError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let this = core::mem::replace(&mut *self, Self::Done);
            match this {
                Self::Setup {
                    extent,
                    alloc,
                    reader,
                } => {
                    let offset = extent.offset.try_into().map_err(|_| {
                        crate::ExtentError::IntegerConversion(crate::TryFromIntError)
                    })?;
                    let len: O = extent.len.try_into().map_err(|_| {
                        crate::ExtentError::IntegerConversion(crate::TryFromIntError)
                    })?;
                    let reserve_len: usize = extent.len.try_into().map_err(|_| {
                        crate::ExtentError::IntegerConversion(crate::TryFromIntError)
                    })?;

                    let extent = crate::Extent { offset, len };

                    let mut buffer = alloc::vec::Vec::new_in(alloc);
                    buffer
                        .try_reserve_exact(reserve_len)
                        .map_err(|_| alloc::alloc::AllocError)?;
                    *self = Self::Reading(ReadingExtent {
                        reader,
                        extent,
                        buffer,
                    })
                }
                Self::Reading(mut r) => match Pin::new(&mut r).poll(cx) {
                    core::task::Poll::Pending => {
                        *self = Self::Reading(r);
                        return core::task::Poll::Pending;
                    }
                    core::task::Poll::Ready(res) => {
                        let _ = res?;
                        return core::task::Poll::Ready(Ok(r.buffer.into_boxed_slice()));
                    }
                },
                Self::Done => panic!("poll after future complete"),
            }
        }
    }
}

impl<'a, O: Offset, R: Unpin + PollReader<O>> PollReader<O> for &'a mut R {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(), IoError>> {
        Pin::new(&mut **self).poll_read(cx, buf)
    }

    fn poll_read_cstr(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<O, IoError>> {
        Pin::new(&mut **self).poll_read_cstr(cx)
    }

    fn seek_start(mut self: Pin<&mut Self>, cx: &mut Context<'_>, amt: O) -> Result<(), IoError> {
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

    #[cfg(feature = "alloc")]
    fn poll_read_extent<A: alloc::alloc::Allocator>(
        mut self: Pin<&mut Self>,
        extent: &crate::Extent<O, O>,
        buf: &mut alloc::vec::Vec<u8, A>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), crate::ExtentError>> {
        Pin::new(&mut **self).poll_read_extent(extent, buf, cx)
    }

    fn poll_read_extent_with(
        mut self: Pin<&mut Self>,
        extent: &crate::Extent<O, O>,
        buf: &mut [u8],
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), crate::IoError>> {
        Pin::new(&mut **self).poll_read_extent_with(extent, buf, cx)
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

    #[cfg(feature = "alloc")]
    fn async_read_bytes_in<
        A: alloc::alloc::Allocator,
        S1: TryInto<O>,
        S2: TryInto<O> + TryInto<usize> + Copy,
    >(
        &mut self,
        extent: crate::Extent<S1, S2>,
        alloc: A,
    ) -> AsyncReadExtent<O, &mut Self, A, S1, S2> {
        AsyncReadExtent::Setup {
            extent,
            alloc,
            reader: self,
        }
    }

    #[cfg(feature = "alloc")]
    fn async_read_bytes<S1: TryInto<O>, S2: TryInto<O> + TryInto<usize> + Copy>(
        &mut self,
        extent: crate::Extent<S1, S2>,
    ) -> AsyncReadExtent<O, &mut Self, alloc::alloc::Global, S1, S2> {
        self.async_read_bytes_in(extent, alloc::alloc::Global)
    }

    fn async_read_extent<'a, S1: TryInto<O>, S2: TryInto<O>>(
        &mut self,
        extent: crate::Extent<S1, S2>,
        buf: &'a mut [u8],
    ) -> AsyncReadExtentWith<'a, O, &mut Self, S1, S2> {
        AsyncReadExtentWith::Setup {
            extent,
            buf,
            reader: self,
        }
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

unsafe impl<O: Offset + Unpin, R: PollReader<O> + Unpin> pin_project::UnsafeUnpin
    for AsyncReadCstr<O, R>
{
}

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

pub enum AsyncReadExtentWith<'a, O: Offset, R: PollReader<O>, S1: TryInto<O>, S2: TryInto<O>> {
    Setup {
        extent: crate::Extent<S1, S2>,
        buf: &'a mut [u8],
        reader: R,
    },
    Reading {
        extent: crate::Extent<O, O>,
        buf: &'a mut [u8],
        reader: R,
    },
    Done,
}

impl<
    'a,
    O: Offset + Unpin,
    R: PollReader<O> + Unpin,
    S1: TryInto<O> + Unpin,
    S2: TryInto<O> + Unpin,
> Future for AsyncReadExtentWith<'a, O, R, S1, S2>
{
    type Output = Result<(), IoError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let this = core::mem::replace(&mut *self, Self::Done);
            match this {
                Self::Setup {
                    extent,
                    buf,
                    reader,
                } => {
                    let offset = extent
                        .offset
                        .try_into()
                        .map_err(|_| IoError::Io(core::io::ErrorKind::FileTooLarge.into()))?;
                    let len: O = extent
                        .len
                        .try_into()
                        .map_err(|_| IoError::Io(core::io::ErrorKind::FileTooLarge.into()))?;

                    let extent = crate::Extent { offset, len };

                    *self = Self::Reading {
                        reader,
                        extent,
                        buf,
                    }
                }
                Self::Reading {
                    mut reader,
                    extent,
                    buf,
                } => match Pin::new(&mut reader).poll_read_extent_with(&extent, buf, cx) {
                    core::task::Poll::Pending => {
                        *self = Self::Reading {
                            reader,
                            extent,
                            buf,
                        };
                        return core::task::Poll::Pending;
                    }
                    core::task::Poll::Ready(res) => {
                        let _ = res?;
                        return core::task::Poll::Ready(Ok(()));
                    }
                },
                Self::Done => panic!("poll after future complete"),
            }
        }
    }
}
