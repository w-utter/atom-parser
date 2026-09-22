use crate::{IoError, Offset, SwapOffsets};

pub trait Reader<O: Offset>: SwapOffsets<O> {
    fn remaining_size(&self) -> O;
    fn offset(&self) -> O;
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError>;
    // reads a cstr and returns its length (not including nul)
    // and advances the cursor after the string
    fn read_cstr(&mut self) -> Result<O, IoError>;
    fn seek(&mut self, amt: O) -> Result<(), IoError>;

    fn seek_remaining(&mut self) -> Result<(), IoError> {
        self.seek(self.remaining_size())
    }
}

impl<'a, O: Offset, R: Reader<O>> Reader<O> for &'a mut R {
    fn remaining_size(&self) -> O {
        R::remaining_size(self)
    }

    fn offset(&self) -> O {
        R::offset(self)
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        R::read(self, bytes)
    }

    fn read_cstr(&mut self) -> Result<O, IoError> {
        R::read_cstr(self)
    }

    fn seek(&mut self, amt: O) -> Result<(), IoError> {
        R::seek(self, amt)
    }
}
