use crate::{ParseOptions, TryFromArraySize};

pub mod async_reader;
pub use async_reader::{AsyncReadCstr, AsyncReader, AsyncSeek, PollReader};
pub mod extensions;
pub use extensions::{BacktrackReader, TrailingReader};
pub mod sync_reader;
pub use sync_reader::Reader;
#[cfg(feature = "provided_readers")]
pub mod provided;
#[cfg(feature = "provided_readers")]
pub use provided::InMemoryReader;

macro_rules! offset_trait {
   ($($t:path),+ $(,)?) => {
       pub trait Offset: $($t+)+ {}
       impl <T: $($t+)+ > Offset for T {}
    };
}

offset_trait! {
    core::cmp::PartialOrd,
    core::cmp::PartialEq,
    num_traits::CheckedSub,
    num_traits::CheckedAdd,
    core::ops::Add,
    num_traits::Zero,
    TryFrom<usize>,
    TryFromArraySize,
    Copy,
}

/// used for nested parsing
/// e.g swapping out the readers offset for another
/// to be able to backtrack / read ahead to a known position
pub trait SwapOffsets<O> {
    fn swap_offsets(&mut self, offset: &mut O);
}

impl<'a, O, S: SwapOffsets<O>> SwapOffsets<O> for &'a mut S {
    fn swap_offsets(&mut self, offset: &mut O) {
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
