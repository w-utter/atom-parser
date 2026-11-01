use crate::FourCC;

pub trait Tag<T>: Sized {
    const TAG: T;
}

/// a subset of tags that represent mp4 atoms
/// used for identifying child atoms
/// during iteration
pub trait Atom: Tag<FourCC> {
    const FCC: FourCC = <Self as Tag<FourCC>>::TAG;
}
