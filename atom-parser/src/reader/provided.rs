use crate::{IoError, SwapOffsets, PollReader, Reader};

pub struct InMemoryReader {
    bytes: Vec<u8>,
    offset: usize,
}

impl InMemoryReader {
    pub fn from_path<P: AsRef<std::path::Path>>(p: P) -> Result<Self, IoError> {
        let bytes = std::fs::read(p)?;
        Ok(Self {
            bytes,
            offset: 0,
        })
    }
}

impl SwapOffsets for InMemoryReader {
    fn swap_offsets(&mut self, offset: &mut usize) {
        core::mem::swap(&mut self.offset, offset)
    }
}

impl Reader for InMemoryReader {
    fn remaining_size(&self) -> usize {
        self.bytes.len().checked_sub(self.offset).unwrap_or_default()
    }

    fn offset(&self) -> usize {
        core::cmp::min(self.offset, self.bytes.len())
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        let remaining = Reader::remaining_size(self);
        if remaining < bytes.len() {
            return Err(IoError::other("not enough spc"));
        }
        bytes.copy_from_slice(&self.bytes[self.offset..self.offset+bytes.len()]);
        self.offset += bytes.len();
        Ok(())
    }

    fn read_cstr(&mut self) -> Result<usize, IoError> {
        let bytes = &self.bytes[self.offset..];
        let len = bytes
            .iter()
            .position(|&byte| byte == b'\0').ok_or(IoError::other("not enough spc"))?;
        self.offset += len + 1;
        Ok(len)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        let remaining = Reader::remaining_size(self);
        if amt > remaining {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.offset += amt;
        Ok(())
    }
}

impl PollReader for InMemoryReader {
    fn poll_read(
        mut self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> core::task::Poll<Result<(), IoError>> {
        let remaining = PollReader::remaining_size(&*self);
        if remaining < buf.len() {
            return core::task::Poll::Ready(Err(IoError::other("not enough spc")));
        }

        buf.copy_from_slice(&self.bytes[self.offset..self.offset+buf.len()]);
        self.offset += buf.len();

        core::task::Poll::Ready(Ok(()))
    }

    fn poll_read_cstr(
        mut self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<usize, IoError>> {
        let bytes = &self.bytes[self.offset..];
        let len = bytes
            .iter()
            .position(|&byte| byte == b'\0').ok_or(IoError::other("not enough spc"))?;
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
            return Err(std::io::Error::other("not enough spc"));
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
        core::cmp::min(self.offset, self.bytes.len())
    }
    fn remaining_size(&self) -> usize {
        self.bytes.len().checked_sub(self.offset).unwrap_or_default()
    }
}
