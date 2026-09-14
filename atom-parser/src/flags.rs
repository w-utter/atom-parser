use crate::{ParseError, ParseOptions};
pub trait FlagsParse<B>: Sized {
    fn try_from_bits(bits: B, options: &ParseOptions) -> Result<Self, ParseError>;
}

pub trait Flags<B> {
    fn from_bits(bits: B) -> Self;
    fn to_bits(self) -> B;
}
