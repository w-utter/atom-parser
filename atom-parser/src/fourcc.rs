use crate::{Offset, Options, PollReader};
pub type FourCC = crate::character_code::CharacterCode<4>;

#[allow(type_alias_bounds)]
pub type FourCCParse<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> =
    crate::character_code::AsyncCharacterCodeParse<4, P, O, R>;

impl FourCC {
    pub const fn from_le_bits(bits: u32) -> Self {
        Self::new(bits.to_le_bytes())
    }

    pub const fn from_be_bits(bits: u32) -> Self {
        Self::new(bits.to_be_bytes())
    }
}
