use crate::{Bits, Options, ParseError};
pub trait FlagsParse<B>: Sized {
    fn try_from_bits<P: Options>(bits: B, options: P) -> Result<Self, ParseError>;
}

// marker trait for types that are represented by flags
pub trait Flags<B>: Bits<Repr = B> {}
