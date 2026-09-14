#![feature(maybe_uninit_array_assume_init)]

pub mod atom_size;
pub use atom_size::{AtomSize, AtomSizeParse};
pub mod integer_parse;
pub use integer_parse::IntegerParse;
pub mod array;
pub use array::{ArrayGuard, ArrayParse, DynamicArray, DynamicArrayIter, AsyncDynamicArrayIter, DynamicArrayParse};
pub mod async_iter_state;
pub use async_iter_state::AsyncIterState;
pub mod fourcc;
pub use fourcc::{FourCC, FourCCParse};
pub mod atom_header;
pub use atom_header::{AtomHeader, AtomHeaderParse};
pub mod reader;
// impl_take_reader is also exported from here
pub use reader::{PollReader, AsyncReader, AsyncReadCstr, AsyncSeek, BacktrackReader, TrailingReader, Reader, SwapOffsets, TakeReader};
//#[cfg(feature = "provided_readers")]
pub use reader::InMemoryReader;
pub mod parse_options;
pub use parse_options::{ParseOptions, Endianess};
pub mod error;
use error::{IoError, ParseError, TryFromIntError};
pub mod children;
pub use children::{Children, SizedChildren, ChildrenIter, AsyncChildrenIter, SizedChildrenParse, ChildrenParse};
pub mod string;
pub use string::{PascalString, NullTerminatedString, PascalStringParse, NullTerminatedStringParse};
pub mod payload;
pub use payload::{Payload, PayloadParse};
pub mod trailing;
pub use trailing::{Trailing, TrailingIterator, TrailingParse, AsyncTrailingIterator};
pub mod parse;
pub use parse::{Parse, AsyncParse};
pub mod flags;
pub use flags::{FlagsParse, Flags};

pub use futures_core::Stream as AsyncIterator;
pub use atom_parser_derive::make_atom;

/// used for identifying child atoms 
/// during iteration
pub trait Atom: Sized {
    const FCC: FourCC;
}

/// offset + Size
pub struct Extent<O, S> {
    pub offset: O,
    pub len: S,
}
