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

    fn extent(&self) -> crate::Extent<O, O> {
        let len = self.remaining_size();
        let offset = self.offset();
        crate::Extent { offset, len }
    }

    #[cfg(feature = "alloc")]
    fn read_extent_in<A: alloc::alloc::Allocator>(
        &self,
        extent: crate::Extent<O, O>,
        alloc: A,
    ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError>;
    #[cfg(feature = "alloc")]
    fn read_bytes_in<A: alloc::alloc::Allocator, S1: TryInto<O>, S2: TryInto<O>>(
        &self,
        extent: crate::Extent<S1, S2>,
        alloc: A,
    ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
        let offset = extent
            .offset
            .try_into()
            .map_err(|_| crate::ExtentError::IntegerConversion(crate::TryFromIntError))?;
        let len = extent
            .len
            .try_into()
            .map_err(|_| crate::ExtentError::IntegerConversion(crate::TryFromIntError))?;
        let extent = crate::Extent { offset, len };
        self.read_extent_in(extent, alloc)
    }

    #[cfg(feature = "alloc")]
    fn read_bytes<S1: TryInto<O>, S2: TryInto<O>>(
        &self,
        extent: crate::Extent<S1, S2>,
    ) -> Result<alloc::boxed::Box<[u8]>, crate::ExtentError> {
        self.read_bytes_in(extent, alloc::alloc::Global)
    }

    fn read_extent_with(&self, extent: crate::Extent<O, O>, buf: &mut [u8]) -> Result<(), IoError>;

    fn read_extent<S1: TryInto<O>, S2: TryInto<O>>(
        &self,
        extent: crate::Extent<S1, S2>,
        buf: &mut [u8],
    ) -> Result<(), IoError> {
        let offset = extent
            .offset
            .try_into()
            .map_err(|_| IoError::Io(core::io::ErrorKind::FileTooLarge.into()))?;
        let len = extent
            .len
            .try_into()
            .map_err(|_| IoError::Io(core::io::ErrorKind::FileTooLarge.into()))?;
        let extent = crate::Extent { offset, len };
        self.read_extent_with(extent, buf)
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

    #[cfg(feature = "alloc")]
    fn read_extent_in<A: alloc::alloc::Allocator>(
        &self,
        extent: crate::Extent<O, O>,
        alloc: A,
    ) -> Result<alloc::boxed::Box<[u8], A>, crate::ExtentError> {
        R::read_extent_in(self, extent, alloc)
    }

    fn read_extent_with(&self, extent: crate::Extent<O, O>, buf: &mut [u8]) -> Result<(), IoError> {
        R::read_extent_with(self, extent, buf)
    }
}
