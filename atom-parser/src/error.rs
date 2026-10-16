#[cfg(feature = "alloc")]
#[derive(thiserror::Error, Debug)]
pub enum ExtentError {
    #[error("allocation error")]
    Alloc(#[from] alloc::alloc::AllocError),
    #[error("io error")]
    Io(#[from] IoError),
    #[error("could not convert between integers")]
    IntegerConversion(#[from] TryFromIntError),
}

#[derive(thiserror::Error, Debug)]
pub enum IoError {
    /// like EOF, except can be for readers with shortened extents
    /// e.g an atom specifies the max amount of data that it contains,
    /// and trying to read out of those bounds during parsing would flag this error
    #[error("unexpected end of data reached")]
    UnexpectedEndOfData,
    #[error("IO error during parse {0:?}")]
    Io(#[from] core::io::Error),
}

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
    #[error("unexpected value from field")]
    UnexpectedValue,
}
