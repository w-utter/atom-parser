// TODO: make this the impl for both flags & enums

pub trait Bits: Sized {
    type Repr;
    fn from_bits(repr: Self::Repr) -> Self;
    fn to_bits(self) -> Self::Repr;
}

macro_rules! nop_bit_impl {
    ($($t:ty,)*) => {
        $(
            impl Bits for $t {
                type Repr = Self;
                fn from_bits(repr: Self::Repr) -> Self {
                    repr
                }
                fn to_bits(self) -> Self::Repr {
                    self
                }
            }
        )*
    }
}

nop_bit_impl! {
    u8, u16, u32, u64,
    i8, i16, i32, i64,
}
