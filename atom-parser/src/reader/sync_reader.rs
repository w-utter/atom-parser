use crate::{IoError, SwapOffsets};

pub trait Reader: SwapOffsets {
    fn remaining_size(&self) -> usize;
    fn offset(&self) -> usize;
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError>;
    // reads a cstr and returns its length (not including nul) 
    // and advances the cursor after the string
    fn read_cstr(&mut self) -> Result<usize, IoError>;
    fn seek(&mut self, amt: usize) -> Result<(), IoError>;

    fn seek_remaining(&mut self) -> Result<(), IoError> {
        self.seek(self.remaining_size())
    }
}

impl <'a, R: Reader> Reader for &'a mut R {
    fn remaining_size(&self) -> usize {
        R::remaining_size(self)
    }

    fn offset(&self) -> usize {
        R::offset(self)
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        R::read(self, bytes)
    }

    fn read_cstr(&mut self) -> Result<usize, IoError> {
        R::read_cstr(self)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        R::seek(self, amt)
    }
}
