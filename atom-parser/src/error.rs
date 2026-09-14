pub type IoError = std::io::Error;

#[derive(thiserror::Error, Debug)]
#[error("could not convert between integers")]
pub struct TryFromIntError;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Atom Size is too small")]
    AtomSizeTooSmall,
    #[cfg(not(feature = "extended_sized_atoms"))]
    #[error("extended atom sizes (64 bytes) is not supported")]
    AtomSizeUnsupported,
    #[error("io error")]
    Io(#[from] IoError),
    #[error("could not convert between integers")]
    IntegerConversion(#[from] TryFromIntError),
    #[error("unknown flags")]
    UnknownFlags,
    #[error("missing expected flags")]
    MissingFlags,
    #[error("reserved field had nonzero bits")]
    UsedReservedField,
}
