use crate::{SwapOffsets, Reader, IoError, PollReader};
use core::task::{Poll, Context};
use core::pin::Pin;

// reader that backtracks to known offsets
// for things like iterating over collections
mod backtrack_reader {
    use super::*;
    pub struct BacktrackReader<R: SwapOffsets> {
        pub reader: R,
        stored_offset: usize,
    }

    impl <R: SwapOffsets> BacktrackReader<R> {
        pub fn new(mut reader: R, mut backtrack_to: usize) -> Self {
            reader.swap_offsets(&mut backtrack_to);
            Self {
                reader,
                stored_offset: backtrack_to,
            }
        }
    }

    impl <R: SwapOffsets> Drop for BacktrackReader<R> {
        fn drop(&mut self) {
            self.reader.swap_offsets(&mut self.stored_offset)
        }
    }

    impl <S: SwapOffsets> SwapOffsets for BacktrackReader<S> {
        fn swap_offsets(&mut self, offset: &mut usize) {
            self.reader.swap_offsets(offset)
        }
    }

    impl <R: Reader> Reader for BacktrackReader<R> {
        fn remaining_size(&self) -> usize {
            self.reader.remaining_size()
        }
        fn offset(&self) -> usize {
            self.reader.offset()
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            self.reader.read(bytes)
        }

        fn read_cstr(&mut self) -> Result<usize, IoError> {
            self.reader.read_cstr()
        }

        fn seek(&mut self, amt: usize) -> Result<(), IoError> {
            self.reader.seek(amt)
        }
    }

    impl <R: PollReader + SwapOffsets + Unpin> PollReader for BacktrackReader<R> {
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
        ) -> Poll<Result<usize, IoError>> {
            Pin::new(&mut self.reader).poll_read_cstr(cx)
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete(cx)
        }

        fn offset(&self) -> usize {
            self.reader.offset()
        }

        fn remaining_size(&self) -> usize {
            self.reader.remaining_size()
        }
    }
}
pub use backtrack_reader::*;

// reader with a capped size
// mostly for nested parsing when sizes are known
// e.g nested atoms
mod trailing_reader {
    pub use super::*;
    pub struct TrailingReader<R> {
        pub reader: R,
        max_offset: usize,
    }

    impl <R> TrailingReader<R> {
        pub fn new(reader: R, max_offset: usize) -> Self {
            Self {
                reader,
                max_offset,
            }
        }
    }

    impl <S: SwapOffsets> SwapOffsets for TrailingReader<S> {
        fn swap_offsets(&mut self, offset: &mut usize) {
            self.reader.swap_offsets(offset)
        }
    }

    impl <R: Reader> Reader for TrailingReader<R> {
        fn remaining_size(&self) -> usize {
            self.max_offset.checked_sub(self.reader.offset()).unwrap_or_default()
        }

        fn offset(&self) -> usize {
            self.reader.offset()
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            if bytes.len() > self.remaining_size() {
                return Err(std::io::Error::other("not enough spc"));
            }
            self.reader.read(bytes)?;
            Ok(())
        }

        fn read_cstr(&mut self) -> Result<usize, IoError> {
            let len = self.reader.read_cstr()?;
            if len > self.remaining_size() {
                return Err(std::io::Error::other("not enough spc"));
            }
            Ok(len)
        }

        fn seek(&mut self, amt: usize) -> Result<(), IoError> {
            if amt > self.remaining_size() {
                return Err(std::io::Error::other("not enough spc"));
            }
            self.reader.seek(amt)?;
            Ok(())
        }
    }

    impl <R: PollReader + Unpin> PollReader for TrailingReader<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            if buf.len() > self.remaining_size() {
                return Poll::Ready(Err(std::io::Error::other("not enough spc")));
            }
            Pin::new(&mut self.reader).poll_read(cx, buf)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<usize, IoError>> {
            let len = core::task::ready!(Pin::new(&mut self.reader).poll_read_cstr(cx))?;

            if len > self.remaining_size() {
                return Poll::Ready(Err(std::io::Error::other("not enough spc")));
            }
            Poll::Ready(Ok(len))
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete(cx)
        }

        fn offset(&self) -> usize {
            self.reader.offset()
        }

        fn remaining_size(&self) -> usize {
            self.max_offset.checked_sub(self.reader.offset()).unwrap_or_default()
        }
    }
}
pub use trailing_reader::*;
