use crate::{CombinedHasher, IoError, Offset, PollReader, Reader, SwapOffsets};
use core::pin::Pin;
use core::task::{Context, Poll};

// reader that backtracks to known offsets
// for things like iterating over collections
mod backtrack_reader {
    use super::*;
    pub struct BacktrackReader<O, R: SwapOffsets<O>> {
        pub reader: R,
        stored_offset: O,
    }

    impl<O, R: SwapOffsets<O>> BacktrackReader<O, R> {
        pub fn new(mut reader: R, mut backtrack_to: O) -> Self {
            reader.swap_offsets(&mut backtrack_to);
            Self {
                reader,
                stored_offset: backtrack_to,
            }
        }
    }

    impl<O, R: SwapOffsets<O>> Drop for BacktrackReader<O, R> {
        fn drop(&mut self) {
            self.reader.swap_offsets(&mut self.stored_offset)
        }
    }

    impl<O, S: SwapOffsets<O>> SwapOffsets<O> for BacktrackReader<O, S> {
        fn swap_offsets(&mut self, offset: &mut O) {
            self.reader.swap_offsets(offset)
        }
    }

    impl<O: Offset, R: Reader<O>> Reader<O> for BacktrackReader<O, R> {
        fn remaining_size(&self) -> O {
            self.reader.remaining_size()
        }
        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            self.reader.read(bytes)
        }

        fn read_cstr(&mut self) -> Result<O, IoError> {
            self.reader.read_cstr()
        }

        fn seek(&mut self, amt: O) -> Result<(), IoError> {
            self.reader.seek(amt)
        }

        #[cfg(feature = "alloc")]
        fn read_extent_in<A: alloc::alloc::Allocator>(
            &self,
            extent: crate::Extent<O, O>,
            alloc: A,
        ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
            self.reader.read_extent_in(extent, alloc)
        }

        fn read_extent_with(
            &self,
            extent: crate::Extent<O, O>,
            buf: &mut [u8],
        ) -> Result<(), IoError> {
            self.reader.read_extent_with(extent, buf)
        }

        fn read_with_hash<H: crate::Hasher>(
            &mut self,
            bytes: &mut [u8],
            hasher: &mut H,
        ) -> Result<(), IoError> {
            self.reader.read_with_hash(bytes, hasher)
        }
        fn seek_with_hash<H: crate::Hasher>(
            &mut self,
            amt: O,
            hasher: &mut H,
        ) -> Result<(), IoError> {
            self.reader.seek_with_hash(amt, hasher)
        }
        fn read_cstr_with_hash<H: crate::Hasher>(&mut self, hasher: &mut H) -> Result<O, IoError> {
            self.reader.read_cstr_with_hash(hasher)
        }
    }

    impl<O: Offset + Unpin, R: PollReader<O> + SwapOffsets<O> + Unpin> PollReader<O>
        for BacktrackReader<O, R>
    {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_read(cx, buf)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<O, IoError>> {
            Pin::new(&mut self.reader).poll_read_cstr(cx)
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: O,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete(cx)
        }

        fn poll_read_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
            hasher: &mut H,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_read_with_hash(cx, buf, hasher)
        }
        fn poll_read_cstr_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            hasher: &mut H,
        ) -> Poll<Result<O, IoError>> {
            Pin::new(&mut self.reader).poll_read_cstr_with_hash(cx, hasher)
        }
        fn seek_start_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: O,
            hasher: &mut H,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start_with_hash(cx, amt, hasher)
        }
        fn poll_seek_complete_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            hasher: &mut H,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete_with_hash(cx, hasher)
        }

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn remaining_size(&self) -> O {
            self.reader.remaining_size()
        }

        #[cfg(feature = "alloc")]
        fn poll_read_extent<A: alloc::alloc::Allocator>(
            mut self: Pin<&mut Self>,
            extent: &crate::Extent<O, O>,
            buf: &mut alloc::vec::Vec<u8, A>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), crate::ExtentError>> {
            Pin::new(&mut self.reader).poll_read_extent(extent, buf, cx)
        }

        fn poll_read_extent_with(
            mut self: Pin<&mut Self>,
            extent: &crate::Extent<O, O>,
            buf: &mut [u8],
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), crate::IoError>> {
            Pin::new(&mut self.reader).poll_read_extent_with(extent, buf, cx)
        }
    }
}
pub use backtrack_reader::*;

// reader with a capped size
// mostly for nested parsing when sizes are known
// e.g nested atoms
mod trailing_reader {
    pub use super::*;
    pub struct TrailingReader<O, R> {
        pub reader: R,
        max_offset: O,
    }

    impl<O, R> TrailingReader<O, R> {
        pub fn new(reader: R, max_offset: O) -> Self {
            Self { reader, max_offset }
        }
    }

    impl<O, S: SwapOffsets<O>> SwapOffsets<O> for TrailingReader<O, S> {
        fn swap_offsets(&mut self, offset: &mut O) {
            self.reader.swap_offsets(offset)
        }
    }

    impl<O: Offset, R: Reader<O>> Reader<O> for TrailingReader<O, R> {
        fn remaining_size(&self) -> O {
            self.max_offset
                .checked_sub(&self.reader.offset())
                .unwrap_or(O::ZERO)
        }

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            let res: Result<O, _> = bytes.len().try_into();
            match res {
                Ok(len) if len > self.remaining_size() => {
                    return Err(IoError::UnexpectedEndOfData);
                }
                Err(_) => {
                    return Err(IoError::Io(core::io::ErrorKind::FileTooLarge.into()));
                }
                _ => (),
            }

            self.reader.read(bytes)?;
            Ok(())
        }

        fn read_cstr(&mut self) -> Result<O, IoError> {
            let len = self.reader.read_cstr()?;
            if len > self.remaining_size() {
                return Err(IoError::UnexpectedEndOfData);
            }
            Ok(len)
        }

        fn seek(&mut self, amt: O) -> Result<(), IoError> {
            if amt > self.remaining_size() {
                return Err(IoError::UnexpectedEndOfData);
            }
            self.reader.seek(amt)?;
            Ok(())
        }

        #[cfg(feature = "alloc")]
        fn read_extent_in<A: alloc::alloc::Allocator>(
            &self,
            extent: crate::Extent<O, O>,
            alloc: A,
        ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
            self.reader.read_extent_in(extent, alloc)
        }

        fn read_extent_with(
            &self,
            extent: crate::Extent<O, O>,
            buf: &mut [u8],
        ) -> Result<(), IoError> {
            self.reader.read_extent_with(extent, buf)
        }

        fn read_with_hash<H: crate::Hasher>(
            &mut self,
            bytes: &mut [u8],
            hasher: &mut H,
        ) -> Result<(), IoError> {
            self.reader.read_with_hash(bytes, hasher)
        }
        fn seek_with_hash<H: crate::Hasher>(
            &mut self,
            amt: O,
            hasher: &mut H,
        ) -> Result<(), IoError> {
            self.reader.seek_with_hash(amt, hasher)
        }
        fn read_cstr_with_hash<H: crate::Hasher>(&mut self, hasher: &mut H) -> Result<O, IoError> {
            self.reader.read_cstr_with_hash(hasher)
        }
    }

    impl<O: Offset + Unpin, R: PollReader<O> + Unpin> PollReader<O> for TrailingReader<O, R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            let res: Result<O, _> = buf.len().try_into();
            match res {
                Ok(len) if len > self.remaining_size() => {
                    return Poll::Ready(Err(IoError::UnexpectedEndOfData));
                }
                Err(_) => {
                    return Poll::Ready(Err(IoError::Io(core::io::ErrorKind::FileTooLarge.into())));
                }
                _ => (),
            }

            Pin::new(&mut self.reader).poll_read(cx, buf)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<O, IoError>> {
            let len = core::task::ready!(Pin::new(&mut self.reader).poll_read_cstr(cx))?;

            if len > self.remaining_size() {
                return Poll::Ready(Err(IoError::UnexpectedEndOfData));
            }
            Poll::Ready(Ok(len))
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: O,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete(cx)
        }

        fn poll_read_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
            hasher: &mut H,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_read_with_hash(cx, buf, hasher)
        }
        fn poll_read_cstr_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            hasher: &mut H,
        ) -> Poll<Result<O, IoError>> {
            Pin::new(&mut self.reader).poll_read_cstr_with_hash(cx, hasher)
        }
        fn seek_start_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: O,
            hasher: &mut H,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start_with_hash(cx, amt, hasher)
        }
        fn poll_seek_complete_with_hash<H: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            hasher: &mut H,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete_with_hash(cx, hasher)
        }

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn remaining_size(&self) -> O {
            self.max_offset
                .checked_sub(&self.reader.offset())
                .unwrap_or(O::ZERO)
        }

        #[cfg(feature = "alloc")]
        fn poll_read_extent<A: alloc::alloc::Allocator>(
            mut self: Pin<&mut Self>,
            extent: &crate::Extent<O, O>,
            buf: &mut alloc::vec::Vec<u8, A>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), crate::ExtentError>> {
            Pin::new(&mut self.reader).poll_read_extent(extent, buf, cx)
        }

        fn poll_read_extent_with(
            mut self: Pin<&mut Self>,
            extent: &crate::Extent<O, O>,
            buf: &mut [u8],
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), crate::IoError>> {
            Pin::new(&mut self.reader).poll_read_extent_with(extent, buf, cx)
        }
    }
}
pub use trailing_reader::*;

impl<O: core::ops::Add<Output = O> + Copy, R: SwapOffsets<O>>
    BacktrackReader<O, TrailingReader<O, R>>
{
    pub fn from_extent(reader: R, extent: crate::Extent<O, O>) -> Self {
        let crate::Extent { offset, len } = extent;
        let max_offset = offset + len;
        BacktrackReader::new(TrailingReader::new(reader, max_offset), offset)
    }
}

mod error_detection_reader {
    pub use super::*;
    pub struct ErrorDetectionReader<H, R> {
        hasher: H,
        reader: R,
    }

    impl<H, R> ErrorDetectionReader<H, R> {
        pub fn new_with(reader: R, hasher: H) -> Self {
            Self { hasher, reader }
        }

        pub fn split(self) -> (H, R) {
            (self.hasher, self.reader)
        }

        pub fn reader(&mut self) -> &mut R {
            &mut self.reader
        }
    }

    impl<H, O, S> SwapOffsets<O> for ErrorDetectionReader<H, S> {
        fn swap_offsets(&mut self, _: &mut O) {
            panic!("swap offsets cannot be used for error detection reader")
        }
    }

    impl<H: crate::Hasher, O: Offset, R: Reader<O>> Reader<O> for ErrorDetectionReader<H, R> {
        fn remaining_size(&self) -> O {
            self.reader.remaining_size()
        }
        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            self.reader.read_with_hash(bytes, &mut self.hasher)
        }

        fn read_cstr(&mut self) -> Result<O, IoError> {
            self.reader.read_cstr_with_hash(&mut self.hasher)
        }

        fn seek(&mut self, amt: O) -> Result<(), IoError> {
            self.reader.seek_with_hash(amt, &mut self.hasher)
        }

        #[cfg(feature = "alloc")]
        fn read_extent_in<A: alloc::alloc::Allocator>(
            &self,
            _: crate::Extent<O, O>,
            _: A,
        ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
            panic!("cannot use read_extent_in with ErrorDetectionReader")
        }

        fn read_extent_with(&self, _: crate::Extent<O, O>, _: &mut [u8]) -> Result<(), IoError> {
            panic!("cannot use read_extent_with with ErrorDetectionReader")
        }

        fn read_with_hash<H2: crate::Hasher>(
            &mut self,
            bytes: &mut [u8],
            hash: &mut H2,
        ) -> Result<(), IoError> {
            let mut combined = CombinedHasher::combine_hashes(&mut self.hasher, hash);
            self.reader.read_with_hash(bytes, &mut combined)
        }
        fn seek_with_hash<H2: crate::Hasher>(
            &mut self,
            amt: O,
            hash: &mut H2,
        ) -> Result<(), IoError> {
            let mut combined = CombinedHasher::combine_hashes(&mut self.hasher, hash);
            self.reader.seek_with_hash(amt, &mut combined)
        }
        fn read_cstr_with_hash<H2: crate::Hasher>(&mut self, hash: &mut H2) -> Result<O, IoError> {
            let mut combined = CombinedHasher::combine_hashes(&mut self.hasher, hash);
            self.reader.read_cstr_with_hash(&mut combined)
        }
    }

    impl<H: crate::hash::FullHasher + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin>
        PollReader<O> for ErrorDetectionReader<H, R>
    {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            Pin::new(reader).poll_read_with_hash(cx, buf, &mut hasher)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<O, IoError>> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            Pin::new(reader).poll_read_cstr_with_hash(cx, &mut hasher)
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: O,
        ) -> Result<(), IoError> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            Pin::new(reader).seek_start_with_hash(cx, amt, &mut hasher)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            Pin::new(reader).poll_seek_complete_with_hash(cx, &mut hasher)
        }

        fn poll_read_with_hash<H2: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
            hash: &mut H2,
        ) -> Poll<Result<(), IoError>> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            let mut combined = CombinedHasher::combine_hashes(&mut hasher, hash);
            Pin::new(reader).poll_read_with_hash(cx, buf, &mut combined)
        }
        fn poll_read_cstr_with_hash<H2: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            hash: &mut H2,
        ) -> Poll<Result<O, IoError>> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            let mut combined = CombinedHasher::combine_hashes(&mut hasher, hash);
            Pin::new(reader).poll_read_cstr_with_hash(cx, &mut combined)
        }

        fn seek_start_with_hash<H2: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: O,
            hash: &mut H2,
        ) -> Result<(), IoError> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            let mut combined = CombinedHasher::combine_hashes(&mut hasher, hash);
            Pin::new(reader).seek_start_with_hash(cx, amt, &mut combined)
        }
        fn poll_seek_complete_with_hash<H2: crate::Hasher>(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            hash: &mut H2,
        ) -> Poll<Result<(), IoError>> {
            let Self { reader, hasher } = &mut *self;
            let mut hasher = hasher;
            let mut combined = CombinedHasher::combine_hashes(&mut hasher, hash);
            Pin::new(reader).poll_seek_complete_with_hash(cx, &mut combined)
        }

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn remaining_size(&self) -> O {
            self.reader.remaining_size()
        }

        #[cfg(feature = "alloc")]
        fn poll_read_extent<A: alloc::alloc::Allocator>(
            self: Pin<&mut Self>,
            _: &crate::Extent<O, O>,
            _: &mut alloc::vec::Vec<u8, A>,
            _: &mut Context<'_>,
        ) -> Poll<Result<(), crate::ExtentError>> {
            panic!("cannot use poll_read_extent with ErrorDetectionReader")
        }

        fn poll_read_extent_with(
            self: Pin<&mut Self>,
            _: &crate::Extent<O, O>,
            _: &mut [u8],
            _: &mut Context<'_>,
        ) -> Poll<Result<(), crate::IoError>> {
            panic!("cannot use poll_read_extent_with with ErrorDetectionReader")
        }
    }
}
pub use error_detection_reader::*;
