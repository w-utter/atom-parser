#[cfg(feature = "alloc")]
mod in_memory {
    use crate::{IoError, PollReader, Reader, SwapOffsets};

    pub enum Bytes<'a, A: alloc::alloc::Allocator = alloc::alloc::Global> {
        Borrowed(&'a [u8]),
        Owned(alloc::vec::Vec<u8, A>),
    }

    impl<'a, A: alloc::alloc::Allocator> Bytes<'a, A> {
        fn bytes(&self) -> &[u8] {
            match self {
                Self::Borrowed(b) => b,
                Self::Owned(o) => o.as_ref(),
            }
        }
    }

    pub struct InMemoryReader<'a, A: alloc::alloc::Allocator = alloc::alloc::Global> {
        bytes: Bytes<'a, A>,
        offset: usize,
    }

    impl InMemoryReader<'_> {
        #[cfg(feature = "std")]
        pub fn from_path<P: AsRef<std::path::Path>>(p: P) -> Result<Self, IoError> {
            let bytes = std::fs::read(p)?;
            Ok(Self {
                bytes: Bytes::Owned(bytes),
                offset: 0,
            })
        }

        pub fn from_bytes<'a>(bytes: &'a [u8]) -> InMemoryReader<'a> {
            InMemoryReader {
                offset: 0,
                bytes: Bytes::Borrowed(bytes),
            }
        }

        pub fn from_buf<A: alloc::alloc::Allocator>(
            buf: alloc::vec::Vec<u8, A>,
        ) -> InMemoryReader<'static, A> {
            InMemoryReader {
                offset: 0,
                bytes: Bytes::Owned(buf),
            }
        }
    }

    impl SwapOffsets<usize> for InMemoryReader<'_> {
        fn swap_offsets(&mut self, offset: &mut usize) {
            core::mem::swap(&mut self.offset, offset)
        }
    }

    impl Reader<usize> for InMemoryReader<'_> {
        fn remaining_size(&self) -> usize {
            self.bytes
                .bytes()
                .len()
                .checked_sub(self.offset)
                .unwrap_or_default()
        }

        fn offset(&self) -> usize {
            core::cmp::min(self.offset, self.bytes.bytes().len())
        }

        fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
            let remaining = Reader::remaining_size(self);
            if remaining < bytes.len() {
                return Err(IoError::UnexpectedEndOfData);
            }
            bytes.copy_from_slice(&self.bytes.bytes()[self.offset..self.offset + bytes.len()]);
            self.offset += bytes.len();
            Ok(())
        }

        fn read_cstr(&mut self) -> Result<usize, IoError> {
            let bytes = &self.bytes.bytes()[self.offset..];
            let len = bytes
                .iter()
                .position(|&byte| byte == b'\0')
                .ok_or(IoError::UnexpectedEndOfData)?;
            self.offset += len + 1;
            Ok(len)
        }

        fn seek(&mut self, amt: usize) -> Result<(), IoError> {
            let remaining = Reader::remaining_size(self);
            if amt > remaining {
                return Err(IoError::UnexpectedEndOfData);
            }
            self.offset += amt;
            Ok(())
        }

        #[cfg(feature = "alloc")]
        fn read_extent_in<A: alloc::alloc::Allocator>(
            &self,
            extent: crate::Extent<usize, usize>,
            alloc: A,
        ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
            if extent.offset + extent.len > self.bytes.bytes().len() {
                return Err(crate::ExtentError::Io(IoError::UnexpectedEndOfData));
            }

            let bytes = &self.bytes.bytes()[extent.offset..(extent.offset + extent.len)];

            let mut v = alloc::vec::Vec::new_in(alloc);
            v.try_reserve_exact(bytes.len())
                .map_err(|_| alloc::alloc::AllocError)?;
            v.extend_from_slice(bytes);
            Ok(v.into_boxed_slice())
        }
    }

    impl PollReader<usize> for InMemoryReader<'_> {
        fn poll_read(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
            buf: &mut [u8],
        ) -> core::task::Poll<Result<(), IoError>> {
            let remaining = PollReader::remaining_size(&*self);
            if remaining < buf.len() {
                return core::task::Poll::Ready(Err(IoError::UnexpectedEndOfData));
            }

            buf.copy_from_slice(&self.bytes.bytes()[self.offset..self.offset + buf.len()]);
            self.offset += buf.len();

            core::task::Poll::Ready(Ok(()))
        }

        fn poll_read_cstr(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<usize, IoError>> {
            let bytes = &self.bytes.bytes()[self.offset..];
            let len = bytes
                .iter()
                .position(|&byte| byte == b'\0')
                .ok_or(IoError::UnexpectedEndOfData)?;
            self.offset += len + 1;
            core::task::Poll::Ready(Ok(len))
        }

        fn seek_start(
            mut self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            let remaining = PollReader::remaining_size(&*self);
            if amt > remaining {
                return Err(IoError::UnexpectedEndOfData);
            }

            self.offset += amt;
            Ok(())
        }

        fn poll_seek_complete(
            self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), IoError>> {
            core::task::Poll::Ready(Ok(()))
        }

        fn offset(&self) -> usize {
            core::cmp::min(self.offset, self.bytes.bytes().len())
        }
        fn remaining_size(&self) -> usize {
            self.bytes
                .bytes()
                .len()
                .checked_sub(self.offset)
                .unwrap_or_default()
        }

        #[cfg(feature = "alloc")]
        fn poll_read_extent<A: alloc::alloc::Allocator>(
            self: core::pin::Pin<&mut Self>,
            extent: &crate::Extent<usize, usize>,
            buf: &mut alloc::vec::Vec<u8, A>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Result<(), crate::ExtentError>> {
            if extent.offset + extent.len > self.bytes.bytes().len() {
                return core::task::Poll::Ready(Err(crate::ExtentError::Io(
                    IoError::UnexpectedEndOfData,
                )));
            }

            let bytes = &self.bytes.bytes()[extent.offset..(extent.offset + extent.len)];
            buf.extend_from_slice(bytes);
            core::task::Poll::Ready(Ok(()))
        }
    }
}

#[cfg(feature = "alloc")]
pub use in_memory::*;
