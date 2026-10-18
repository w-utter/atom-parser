use crate::{ParseOptions, TryFromArraySize};

pub mod async_reader;
pub use async_reader::{AsyncReadCstr, AsyncReader, AsyncSeek, PollReader};
pub mod extensions;
pub use extensions::{BacktrackReader, TrailingReader};
pub mod sync_reader;
pub use sync_reader::Reader;
#[cfg(feature = "provided_readers")]
pub mod provided;
#[cfg(all(feature = "provided_readers", feature = "alloc"))]
pub use provided::InMemoryReader;

#[cfg(all(feature = "provided_readers", feature = "std", unix))]
pub use provided::{AsyncFileReader, FileReader};

macro_rules! offset_trait {
   ($($t:path),+ $(,)?) => {
       pub trait Offset: $($t+)+ {}
       impl <T: $($t+)+ > Offset for T {}
    };
}

offset_trait! {
    core::cmp::PartialOrd,
    core::cmp::PartialEq,
    core::ops::Add<Output = Self>,
    CheckedArithmetic,
    TryFrom<usize>,
    TryInto<usize>,
    TryFromArraySize,
    Copy,
}

macro_rules! checked_arithmetic {
    ($($t:ty),* $(,)?) => {
        pub trait CheckedArithmetic: Sized {
            const ZERO: Self;
            fn checked_add(&self, rhs: &Self) -> Option<Self>;
            fn checked_sub(&self, rhs: &Self) -> Option<Self>;
        }

        $(
            impl CheckedArithmetic for $t {
                const ZERO: Self = 0;
                fn checked_add(&self, rhs: &Self) -> Option<Self> {
                    <$t>::checked_add(*self, *rhs)
                }
                fn checked_sub(&self, rhs: &Self) -> Option<Self> {
                    <$t>::checked_sub(*self, *rhs)
                }
            }
        )*
    }
}

checked_arithmetic! {
    u8, u16, u32, u64, usize,
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
