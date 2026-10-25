use crate::{Bits, ParseError, ParseOptions};
pub trait FlagsParse<B>: Sized {
    fn try_from_bits(bits: B, options: &ParseOptions) -> Result<Self, ParseError>;
}

// marker trait for types that are represented by flags
pub trait Flags<B>: Bits<Repr = B> {}
