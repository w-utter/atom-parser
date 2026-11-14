use crate::character_set::CharacterSet;
use crate::info::InfoList;
use atom_parser::{FourCC, make_atom};

impl crate::RiffDerived for Entry {
    const FILE_TYPE: FourCC = FourCC::new(*b"WAVE");
}

pub type WavFile = crate::RiffDerivedHeader<Entry>;

make_atom! {
    #[derive(Debug)]
    struct Entry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        entry_kind: enum Entry {
            ListEntry,

            CharacterSet,
            Format,
            Fact,
            Cue,
            Playlist,
            Data,
        }
    }
}

// list
make_atom! {
    #[tag(FourCC, FourCC::new(*b"LIST"))]
    #[derive(Debug)]
    struct ListEntry {
        #[tag]
        list_type: FourCC,
        #[tagged]
        ty: enum List {
            crate::List as GenericList,

            InfoList as Info,
            AssociatedDataList as AssociatedData,
            WaveList as Wave,
        }
    }
}

make_atom! {
    #[derive(Debug)]
    #[enum_repr(u16)]
    enum AudioFormat {
        Pcm = 0x0001,
        MuLaw = 0x0101,
        ALaw = 0x0102,
        Adpcm = 0x0103,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"fmt "))]
    struct Format {
        format: AudioFormat,
        channels: u16,
        sample_rate: u32,
        bytes_per_sec: u32,
        block_size: u16,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"fact"))]
    struct Fact {
        sample_count: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"cue "))]
    struct Cue {
        name: u32,
        position: u32,
        chunk: FourCC,
        chunk_start: u32,
        block_start: u32,
        sample_offset: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    struct PlaySegment {
        name: u32,
        sample_length: u32,
        repeat_count: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"plst"))]
    struct Playlist {
        #[dynamic_array(size_type = u32, zero_relative = false)]
        segments: PlaySegment,
    }
}

make_atom! {
    #[derive(Debug)]
    struct AssociatedDataEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        data: enum AssociatedData {
            crate::List as GenericList,

            Label,
            Note,
            Text,
            File,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"adtl"))]
    struct AssociatedDataList {
        #[trailing_array]
        entries: AssociatedDataEntry,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"labl"))]
    struct Label {
        name: u32,
        #[null_terminated_string]
        label: String,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"note"))]
    struct Note {
        name: u32,
        #[null_terminated_string]
        comment: String,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"ltxt"))]
    struct Text {
        name: u32,
        sample_length: u32,
        purpose: u32,
        country: u16,
        language: u16,
        dialect: u16,
        code_page: u16,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"file"))]
    struct File {
        name: u32,
        media_type: FourCC,
        #[payload]
        file_data: Vec<u8>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"slnt"))]
    struct Silence {
        silent_samples: u16,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"data"))]
    struct Data {
        #[payload]
        bytes: Vec<u8>,
    }

    #[derive(Debug)]
    struct WaveEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        entry: enum EntryKind {
            crate::List as GenericList,

            Data,
            Silence,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"wavl"))]
    struct WaveList {
        #[trailing_array]
        entries: WaveEntry,
    }
}

#[cfg(test)]
const TEST_FILE: &str = "../M1F1-Alaw-AFsp.wav";

#[test]
fn wav() {
    #[cfg(feature = "std")]
    let mut r = atom_parser::FileReader::<_, 1024>::from_path(TEST_FILE).unwrap();
    #[cfg(not(feature = "std"))]
    let mut r = {
        let bytes = std::fs::read(TEST_FILE).unwrap();
        atom_parser::InMemoryReader::from_buf(bytes)
    };
    let opts = atom_parser::ParseOptions {
        endianess: atom_parser::Endianess::Little,
        ..Default::default()
    };
    use atom_parser::Parse;
    let wav = WavFile::parse(&mut r, &opts).unwrap();
    println!("wav: {wav:?}");

    let mut entry_iter = wav.entries(&mut r, &opts).unwrap();
    while let Some(entry) = entry_iter.next() {
        let entry = entry.unwrap();
        let reader = &mut entry_iter.reader();
        use entry::Entry;
        match entry.entry_kind {
            Entry::ListEntry(list) => {
                use list_entry::List;
                match list.ty {
                    List::GenericList(g) => println!("list: {g:?}"),
                    List::Info(i) => {
                        let mut info_iter = i.entries(reader, &opts).unwrap();
                        while let Some(info) = info_iter.next() {
                            let info = info.unwrap();
                            println!("info: {info:?}");
                        }
                    }
                    List::AssociatedData(assoc) => {
                        let mut assoc_iter = assoc.entries(reader, &opts).unwrap();
                        while let Some(assoc) = assoc_iter.next() {
                            let assoc = assoc.unwrap();
                            println!("assoc: {assoc:?}");
                        }
                    }
                    List::Wave(wave) => {
                        let mut wave_iter = wave.entries(reader, &opts).unwrap();
                        while let Some(wave) = wave_iter.next() {
                            let wave = wave.unwrap();
                            println!("wave: {wave:?}");
                        }
                    }
                    List::Unsupported(_) => (),
                }
            }
            Entry::CharacterSet(cset) => println!("character set: {cset:?}"),
            Entry::Format(f) => println!("fmt: {f:?}"),
            Entry::Fact(f) => println!("fact: {f:?}"),
            Entry::Cue(c) => println!("cue: {c:?}"),
            Entry::Playlist(p) => {
                let segments = p
                    .segments(reader, &opts)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                println!("playlist segments: {segments:?}");
            }
            Entry::Data(d) => println!("data: {d:?}"),
            Entry::Unsupported(_) => (),
        }
    }
    panic!()
}

#[tokio::test]
async fn wav_async() {
    #[cfg(feature = "std")]
    let mut r = {
        let file = tokio::fs::File::open(TEST_FILE).await.unwrap();
        let size = file.metadata().await.unwrap().len();
        let file = std::sync::Arc::new(file.into_std().await);
        atom_parser::reader::AsyncFileReader::<_, 1024>::new(file, size)
    };
    #[cfg(not(feature = "std"))]
    let mut r = {
        let bytes = std::fs::read(TEST_FILE).unwrap();
        atom_parser::InMemoryReader::from_buf(bytes)
    };
    let opts = atom_parser::ParseOptions {
        endianess: atom_parser::Endianess::Little,
        ..Default::default()
    };
    use atom_parser::AsyncParse;
    let wav = WavFile::parse_async(&mut r, &opts).await.unwrap();
    println!("wav: {wav:?}");

    let mut entry_iter = wav.entries_async(&mut r, &opts).unwrap();
    use tokio_stream::StreamExt;
    while let Some(entry) = entry_iter.next().await {
        let entry = entry.unwrap();
        let reader = &mut entry_iter.reader();
        use entry::Entry;
        match entry.entry_kind {
            Entry::ListEntry(list) => {
                use list_entry::List;
                match list.ty {
                    List::GenericList(g) => println!("list: {g:?}"),
                    List::Info(i) => {
                        let mut info_iter = i.entries_async(reader, &opts).unwrap();
                        while let Some(info) = info_iter.next().await {
                            let info = info.unwrap();
                            println!("info: {info:?}");
                        }
                    }
                    List::AssociatedData(assoc) => {
                        let mut assoc_iter = assoc.entries_async(reader, &opts).unwrap();
                        while let Some(assoc) = assoc_iter.next().await {
                            let assoc = assoc.unwrap();
                            println!("assoc: {assoc:?}");
                        }
                    }
                    List::Wave(wave) => {
                        let mut wave_iter = wave.entries_async(reader, &opts).unwrap();
                        while let Some(wave) = wave_iter.next().await {
                            let wave = wave.unwrap();
                            println!("wave: {wave:?}");
                        }
                    }
                    List::Unsupported(_) => (),
                }
            }
            Entry::CharacterSet(cset) => println!("character set: {cset:?}"),
            Entry::Format(f) => println!("fmt: {f:?}"),
            Entry::Fact(f) => println!("fact: {f:?}"),
            Entry::Cue(c) => println!("cue: {c:?}"),
            Entry::Playlist(p) => {
                let segments = p
                    .segments_async(reader, &opts)
                    .collect::<Result<Vec<_>, _>>()
                    .await;
                println!("playlist segments: {segments:?}");
            }
            Entry::Data(d) => println!("data: {d:?}"),
            Entry::Unsupported(_) => (),
        }
    }
    panic!()
}
