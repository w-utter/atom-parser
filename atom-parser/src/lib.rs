#![feature(maybe_uninit_array_assume_init)]
#![feature(core_io)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod atom_size;
pub use atom_size::{AtomSize, AtomSizeParse};
pub mod integer_parse;
pub use integer_parse::IntegerParse;
pub mod array;
pub use array::{
    ArrayGuard, ArrayParse, ArraySize, AsyncDynamicArrayIter, DynamicArray, DynamicArrayIter,
    DynamicArrayParse, NullTerminated, TryFromArraySize,
};
pub mod async_iter_state;
pub use async_iter_state::AsyncIterState;
pub mod fourcc;
pub use fourcc::{FourCC, FourCCParse};
pub mod atom_header;
pub use atom_header::{AtomHeader, AtomHeaderParse};
pub mod reader;
// impl_take_reader is also exported from here
#[cfg(all(feature = "provided_readers", feature = "alloc"))]
pub use reader::InMemoryReader;
#[cfg(all(feature = "provided_readers", feature = "std", unix))]
pub use reader::{AsyncFileReader, FileReader};
pub use reader::{
    AsyncReadCstr, AsyncReader, AsyncSeek, BacktrackReader, Offset, PollReader, Reader,
    SwapOffsets, TakeReader, TrailingReader,
};
pub mod parse_options;
pub use parse_options::{Endianess, ParseOptions};
pub mod error;
#[cfg(feature = "alloc")]
pub use error::ExtentError;
pub use error::{IoError, ParseError, TryFromIntError};

pub mod children;
pub use children::{
    AsyncChildrenIter, Children, ChildrenIter, ChildrenParse, SizedChildren, SizedChildrenParse,
};
pub mod string;
pub use string::{
    NullTerminatedString, NullTerminatedStringParse, PascalString, PascalStringParse,
};
pub mod payload;
pub use payload::{Payload, PayloadParse};
pub mod trailing;
pub use trailing::{AsyncTrailingIterator, Trailing, TrailingIterator, TrailingParse};
pub mod parse;
pub use parse::{AsyncParse, Parse, Parsed};
pub mod flags;
pub use flags::{Flags, FlagsParse};
pub mod version;
pub use version::UnknownVersion;
pub mod atom;
pub use atom::AnonymousAtom;
pub mod bits;
pub use bits::Bits;

pub use atom_parser_derive::make_atom;
pub use futures_core::Stream as AsyncIterator;

/// used for identifying child atoms
/// during iteration
pub trait Atom: Sized {
    const FCC: FourCC;
}

/// offset + Size
#[derive(Debug, Clone, Copy)]
pub struct Extent<O, S> {
    pub offset: O,
    pub len: S,
}
