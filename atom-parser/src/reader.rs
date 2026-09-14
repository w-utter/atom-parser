use crate::ParseOptions;

pub mod async_reader;
pub use async_reader::{AsyncReadCstr, AsyncReader, AsyncSeek, PollReader};
pub mod extensions;
pub use extensions::{BacktrackReader, TrailingReader};
pub mod sync_reader;
pub use sync_reader::Reader;
//#[cfg(feature = "provided_readers")]
pub mod provided;
//#[cfg(feature = "provided_readers")]
pub use provided::InMemoryReader;

/// used for nested parsing
/// e.g swapping out the readers offset for another
/// to be able to backtrack / read ahead to a known position
pub trait SwapOffsets {
    fn swap_offsets(&mut self, offset: &mut usize);
}

impl<'a, S: SwapOffsets> SwapOffsets for &'a mut S {
    fn swap_offsets(&mut self, offset: &mut usize) {
        S::swap_offsets(self, offset)
    }
}

/// used for stealing the current reader in an async state machine
pub trait TakeReader<'a, R> {
    fn take_reader(self) -> (R, &'a ParseOptions);
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions);
}

#[macro_export]
macro_rules! impl_take_reader {
    () => {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            match self {
                Self::Done(reader, opts) => return (reader, opts),
                _ => unreachable!("invalid state"),
            }
        }
    };
}

pub use impl_take_reader;
