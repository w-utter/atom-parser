use crate::{IoError, Offset, PollReader, Reader, SwapOffsets};
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

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn remaining_size(&self) -> O {
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
                .unwrap_or(O::zero())
        }

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            let res: Result<O, _> = bytes.len().try_into();
            match res {
                Ok(len) if len > self.remaining_size() => {
                    return Err(std::io::Error::other("not enough spc"));
                }
                Err(_) => {
                    return Err(std::io::Error::other("too big"));
                }
                _ => (),
            }

            self.reader.read(bytes)?;
            Ok(())
        }

        fn read_cstr(&mut self) -> Result<O, IoError> {
            let len = self.reader.read_cstr()?;
            if len > self.remaining_size() {
                return Err(std::io::Error::other("not enough spc"));
            }
            Ok(len)
        }

        fn seek(&mut self, amt: O) -> Result<(), IoError> {
            if amt > self.remaining_size() {
                return Err(std::io::Error::other("not enough spc"));
            }
            self.reader.seek(amt)?;
            Ok(())
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
                    return Poll::Ready(Err(std::io::Error::other("not enough spc")));
                }
                Err(_) => {
                    return Poll::Ready(Err(std::io::Error::other("too big")));
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
                return Poll::Ready(Err(std::io::Error::other("not enough spc")));
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

        fn offset(&self) -> O {
            self.reader.offset()
        }

        fn remaining_size(&self) -> O {
            self.max_offset
                .checked_sub(&self.reader.offset())
                .unwrap_or(O::zero())
        }
    }
}
pub use trailing_reader::*;
