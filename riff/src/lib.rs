pub mod avi;
pub mod character_set;
pub mod info;
pub mod wav;
pub mod webp;

use atom_parser::{FourCC, make_atom};

pub trait RiffDerived {
    const FILE_TYPE: FourCC;
}

make_atom! {
    #[derive(Debug)]
    struct RiffFile {
        #[expected(FourCC::new(*b"RIFF"))]
        fourcc: FourCC,
        // offset from the size of `file_type`
        #[payload_length(offset = 4, padding = 2)]
        file_size: u32,
        file_type: FourCC,
        #[trailing_array]
        entries: Entry,
    }

    #[derive(Debug)]
    struct RiffDerivedHeader<T: RiffDerived> {
        #[expected(FourCC::new(*b"RIFF"))]
        fourcc: FourCC,
        // offset from the size of `file_type`
        #[payload_length(offset = 4, padding = 2)]
        file_size: u32,
        #[expected(<T as RiffDerived>::FILE_TYPE)]
        file_type: FourCC,
        #[trailing_array]
        entries: T,
    }

    #[derive(Debug)]
    struct Entry {
        #[condition]
        kind: FourCC,
        #[conditional(EntryKind)]
        entry_kind: {
            if kind == <List as atom_parser::Tag<FourCC>>::TAG {
                ListEntry {
                    #[payload_length(padding = 2)]
                    size: u32,
                    #[padded]
                    list: List,
                }
            } else {
                Chunk {
                    #[payload_length(padding = 2)]
                    size: u32,
                    #[reuse_condition]
                    kind: FourCC,
                    #[payload]
                    data: Vec<u8>,
                }
            }
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"LIST"))]
    struct List {
        list_type: FourCC,
        #[trailing_array]
        data: Entry,
    }
}
