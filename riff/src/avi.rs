use crate::character_set::CharacterSet;
use crate::info::InfoList;
use atom_parser::{FourCC, make_atom};

pub type TwoCC = atom_parser::CharacterCode<2>;

impl crate::RiffDerived for Entry {
    const FILE_TYPE: FourCC = FourCC::new(*b"AVI ");
}

pub type AviFile = crate::RiffDerivedHeader<Entry>;

make_atom! {
    #[derive(Debug)]
    struct Entry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        entry_kind: enum Entry {
            BaseListEntry,

            AviIndex,
            CharacterSet,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"LIST"))]
    struct BaseListEntry {
        #[tag]
        list_type: FourCC,
        #[tagged]
        ty: enum BaseList {
            InfoList,

            AviFormatList,
            MovieList,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"hdrl"))]
    struct AviFormatList {
        #[trailing_array]
        entries: AviFormatEntry,
    }

    #[derive(Debug)]
    struct AviFormatEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        data: enum AviFormat {
            AviHeader,
            StreamList,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"LIST"))]
    struct StreamList {
        #[tag]
        list_type: FourCC,
        #[tagged]
        ty: enum List {
            InfoList,

            Stream,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"movi"))]
    struct MovieList {
        #[trailing_array]
        entries: MovieEntry,
    }

    #[derive(Debug)]
    struct MovieEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        data: enum MovieData {
            MovieDataList,

            #[tag_pattern(atom_parser::CharacterCode([_, _, b'w', b'b']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            AudioData,
            #[tag_pattern(atom_parser::CharacterCode([_, _, b'd', b'b']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            UncompressedVideoData,
            #[tag_pattern(atom_parser::CharacterCode([_, _, b'd', b'c']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            CompressedVideoData,
            #[tag_pattern(atom_parser::CharacterCode([_, _, b'p', b'c']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            PaletteChange,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"LIST"))]
    struct MovieDataList {
        #[tag]
        list_type: FourCC,
        #[tagged]
        ty: enum List {
            crate::List as GenericList,

            RecordList as Record,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"rec "))]
    struct RecordList {
        #[trailing_array]
        entries: RecordListEntry,
    }

    #[derive(Debug)]
    struct RecordListEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        data: enum Record {
            crate::List as GenericList,

            #[tag_pattern(atom_parser::CharacterCode([_, _, b'w', b'b']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            AudioData,
            #[tag_pattern(atom_parser::CharacterCode([_, _, b'd', b'b']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            UncompressedVideoData,
            #[tag_pattern(atom_parser::CharacterCode([_, _, b'd', b'c']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            CompressedVideoData,
            #[tag_pattern(atom_parser::CharacterCode([_, _, b'p', b'c']))]
            #[rebind_tag(TwoCC, FourCC::prefix::<2>)]
            PaletteChange,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"idx1"))]
    struct AviIndex {
        chunk_id: FourCC,
        #[flags(u32)]
        flags: struct Flags {
            const LIST = 1 << 0;
            const KEYFRAME = 1 << 1;
            const FIRST_PART = 1 << 2;
            const LAST_PART = 1 << 3;
            const NO_TIME = 1 << 4;
        },
        chunk_offset: u32,
        chunk_length: u32,
    }
}

// format specific
make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"avih"))]
    struct AviHeader {
        us_per_frame: u32,
        max_bytes_per_sec: u32,
        padding_granularity: u32,
        #[flags(u32)]
        flags: struct Flags {
            const HAS_INDEX = 1 << 0;
            const MUST_USE_INDEX = 1 << 1;
            const INTERLEAVED = 1 << 2;
            const CAPTURE_FILE = 1 << 3;
            const COPYRIGHTED = 1 << 4;
        },
        total_frames: u32,
        initial_frames: u32,
        streams: u32,
        suggested_buffer_size: u32,
        width: u32,
        height: u32,
        scale: u32,
        rate: u32,
        start: u32,
        length: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"strl"))]
    struct Stream {
        #[trailing_array]
        entries: StreamEntry,
    }

    #[derive(Debug)]
    struct StreamEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        data: enum Stream {
            crate::List as GenericList,

            StreamHeader,
            StreamFormat,
            StreamData,
        }
    }

    #[derive(Debug)]
    #[enum_repr(FourCC)]
    enum StreamType {
        Video = FourCC::new(*b"vids"),
        Audio = FourCC::new(*b"auds"),
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"strh"))]
    struct StreamHeader {
        stream_type: StreamType,
        codec: FourCC,
        #[flags(u32)]
        flags: struct Flags {
            const DISABLED = 1 << 0;
            const EMBEDDED_PALETTE_CHANGES = 1 << 1;
        },
        #[reserved]
        reserved: [u8; 4],
        initial_frames: u32,
        scale: u32,
        rate: u32,
        start: u32,
        length: u32,
        suggested_buffer_size: u32,
        quality: u32,
        sample_size: u32,
    }

    // data is dependent on the stream type
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"strf"))]
    struct StreamFormat {
        #[payload]
        data: Vec<u8>,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"strd"))]
    struct StreamData {
        #[payload]
        data: Vec<u8>,
    }
}

make_atom! {
    // has last 2 bytes in its fourcc as 'wb'
    #[derive(Debug)]
    struct AudioData {
        #[payload]
        data: Vec<u8>,
    }

    // has last 2 bytes in its fourcc as 'db'
    #[derive(Debug)]
    struct UncompressedVideoData {
        #[payload]
        data: Vec<u8>,
    }


    // has last 2 bytes in its fourcc as 'dc'
    #[derive(Debug)]
    struct CompressedVideoData {
        #[payload]
        data: Vec<u8>,
    }
}

make_atom! {
    // has last 2 bytes in its fourcc as 'pc'
    #[derive(Debug)]
    struct PaletteChange {
        first_entry: u8,
        entry_count: u32,
        #[reserved]
        flags: [u32; 4],
        #[trailing_array]
        palette_entries: PaletteEntry,
    }

    #[derive(Debug)]
    struct PaletteEntry {
        rgb: [u8; 3],
        #[flags(u8)]
        flags: struct Flags {
            const EXPLICIT = 1 << 0;
            const NO_COLLAPSE = 1 << 1;
            const ANIMATION = 1 << 2;
        }
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"JUNK"))]
    struct Junk {
        #[payload]
        junk: Vec<u8>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[enum_repr(u16)]
    enum BitCount {
        ImpliedByFormat = 0,
        MonoChrome = 1,
        Max16Colors = 4,
        Max256Colors = 8,
        Max65KColors = 16,
        Max16MColors = 24,
        Max4BColors = 32,
    }

    #[derive(Debug)]
    struct VideoStreamFormat {
        #[expected(40)]
        size: u32,
        width: u32,
        height: u32,
        planes: u16,
        bitcount: BitCount,
        compression: FourCC,
        image_size: u32,
        pixels_per_meter_x: u32,
        pixels_per_meter_y: u32,
        color_table_indexes_used: u32,
        color_indexes_required: u32,
    }

    #[derive(Debug)]
    struct AudioStreamFormat {
        format_tag: u16,
        channels: u16,
        samples_per_sec: u32,
        avg_bytes_per_sec: u32,
        block_align: u16,
        bits_per_sample: u16,
        #[payload_length]
        cb_size: u16,
        #[payload]
        extra_format_information: Vec<u8>,
    }
}

#[derive(Debug)]
pub struct AviStream<O> {
    pub header: StreamHeader<O>,
    pub format: AviStreamFormat<O>,
}

#[derive(Debug)]
pub enum AviStreamFormat<O> {
    Audio(AudioStreamFormat<O>),
    Video(VideoStreamFormat<O>),
}

#[derive(Debug)]
pub struct AviInfo<O> {
    pub header: AviHeader<O>,
    pub streams: Vec<AviStream<O>>,
}

#[cfg(test)]
const TEST_FILE: &str = "../file_example_AVI_480_750kB.avi";

#[test]
fn avi() {
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
    let avi = AviFile::parse(&mut r, &opts).unwrap();
    println!("avi: {avi:?}");

    let mut avi_info = None;

    let mut entry_iter = avi.entries(&mut r, &opts).unwrap();

    while let Some(entry) = entry_iter.next() {
        let entry = entry.unwrap();
        let reader = &mut entry_iter.reader();
        use entry::Entry;
        match entry.entry_kind {
            Entry::BaseListEntry(list) => {
                use base_list_entry::BaseList;
                match list.ty {
                    BaseList::InfoList(info) => {
                        let mut info_iter = info.entries(reader, &opts).unwrap();
                        while let Some(info) = info_iter.next() {
                            let info = info.unwrap();
                            println!("info: {info:?}");
                        }
                    }
                    BaseList::AviFormatList(fmt) => {
                        let mut fmt_iter = fmt.entries(reader, &opts).unwrap();

                        let mut avi_header = None;
                        let mut streams = vec![];

                        while let Some(fmt) = fmt_iter.next() {
                            let reader = &mut fmt_iter.reader();
                            let fmt = fmt.unwrap();

                            use avi_format_entry::AviFormat;
                            match fmt.data {
                                AviFormat::AviHeader(header) => {
                                    avi_header = Some(header);
                                }
                                AviFormat::StreamList(list) => {
                                    use stream_list::List;

                                    let mut stream_header = None;
                                    match list.ty {
                                        List::InfoList(info) => {
                                            println!("info: {info:?}");
                                        }
                                        List::Stream(stream) => {
                                            let mut stream_iter =
                                                stream.entries(reader, &opts).unwrap();
                                            while let Some(stream) = stream_iter.next() {
                                                let stream = stream.unwrap();
                                                let reader = &mut stream_iter.reader();

                                                use stream_entry::Stream;
                                                match stream.data {
                                                    Stream::StreamHeader(header) => {
                                                        assert!(
                                                            stream_header.is_none(),
                                                            "multiple headers without format"
                                                        );
                                                        stream_header = Some(header);
                                                    }
                                                    Stream::StreamFormat(fmt) => {
                                                        let header =
                                                            core::mem::take(&mut stream_header)
                                                                .expect(
                                                                    "no header for stream format",
                                                                );
                                                        let mut fmt_reader = atom_parser::BacktrackReader::from_extent(
                                                            reader,
                                                            fmt.data.extent(),
                                                        );
                                                        let format = match header.stream_type {
                                                            StreamType::Video => {
                                                                AviStreamFormat::Video(
                                                                    VideoStreamFormat::parse(
                                                                        &mut fmt_reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap(),
                                                                )
                                                            }
                                                            StreamType::Audio => {
                                                                AviStreamFormat::Audio(
                                                                    AudioStreamFormat::parse(
                                                                        &mut fmt_reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap(),
                                                                )
                                                            }
                                                            _ => continue,
                                                        };
                                                        streams.push(AviStream { header, format });
                                                    }
                                                    _ => (),
                                                }
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                                _ => (),
                            }
                        }
                        avi_info = Some(AviInfo {
                            header: core::mem::take(&mut avi_header)
                                .expect("no avi header for stream"),
                            streams,
                        });
                    }
                    BaseList::MovieList(movi) => {
                        let stream_info =
                            avi_info.as_ref().expect("no stream info for stream data");
                        println!("stream info: {stream_info:?}");
                        let mut movi_iter = movi.entries(reader, &opts).unwrap();
                        while let Some(movi) = movi_iter.next() {
                            let movi = movi.unwrap();
                            println!("movi: {movi:?}");
                        }
                    }
                    _ => (),
                }
            }
            Entry::CharacterSet(cset) => {
                println!("cset: {cset:?}");
            }
            Entry::AviIndex(index) => {
                println!("index: {index:?}");
            }
            _ => (),
        }
    }
    panic!();
}

#[tokio::test]
async fn avi_async() {
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

    let avi = AviFile::parse_async(&mut r, &opts).await.unwrap();
    println!("avi: {avi:?}");

    let mut avi_info = None;

    let mut entry_iter = avi.entries_async(&mut r, &opts).unwrap();

    use tokio_stream::StreamExt;

    while let Some(entry) = entry_iter.next().await {
        let entry = entry.unwrap();
        let reader = &mut entry_iter.reader();
        use entry::Entry;
        match entry.entry_kind {
            Entry::BaseListEntry(list) => {
                use base_list_entry::BaseList;
                match list.ty {
                    BaseList::InfoList(info) => {
                        let mut info_iter = info.entries_async(reader, &opts).unwrap();
                        while let Some(info) = info_iter.next().await {
                            let info = info.unwrap();
                            println!("info: {info:?}");
                        }
                    }
                    BaseList::AviFormatList(fmt) => {
                        let mut fmt_iter = fmt.entries_async(reader, &opts).unwrap();

                        let mut avi_header = None;
                        let mut streams = vec![];

                        while let Some(fmt) = fmt_iter.next().await {
                            let reader = &mut fmt_iter.reader();
                            let fmt = fmt.unwrap();

                            use avi_format_entry::AviFormat;
                            match fmt.data {
                                AviFormat::AviHeader(header) => {
                                    avi_header = Some(header);
                                }
                                AviFormat::StreamList(list) => {
                                    use stream_list::List;

                                    let mut stream_header = None;
                                    match list.ty {
                                        List::InfoList(info) => {
                                            println!("info: {info:?}");
                                        }
                                        List::Stream(stream) => {
                                            let mut stream_iter =
                                                stream.entries_async(reader, &opts).unwrap();
                                            while let Some(stream) = stream_iter.next().await {
                                                let stream = stream.unwrap();
                                                let reader = &mut stream_iter.reader();

                                                use stream_entry::Stream;
                                                match stream.data {
                                                    Stream::StreamHeader(header) => {
                                                        assert!(
                                                            stream_header.is_none(),
                                                            "multiple headers without format"
                                                        );
                                                        stream_header = Some(header);
                                                    }
                                                    Stream::StreamFormat(fmt) => {
                                                        let header =
                                                            core::mem::take(&mut stream_header)
                                                                .expect(
                                                                    "no header for stream format",
                                                                );
                                                        let mut fmt_reader = atom_parser::BacktrackReader::from_extent(
                                                            reader,
                                                            fmt.data.extent(),
                                                        );
                                                        let format = match header.stream_type {
                                                            StreamType::Video => {
                                                                AviStreamFormat::Video(
                                                                    VideoStreamFormat::parse_async(
                                                                        &mut fmt_reader,
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap(),
                                                                )
                                                            }
                                                            StreamType::Audio => {
                                                                AviStreamFormat::Audio(
                                                                    AudioStreamFormat::parse_async(
                                                                        &mut fmt_reader,
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap(),
                                                                )
                                                            }
                                                            _ => continue,
                                                        };
                                                        streams.push(AviStream { header, format });
                                                    }
                                                    _ => (),
                                                }
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                                _ => (),
                            }
                        }
                        avi_info = Some(AviInfo {
                            header: core::mem::take(&mut avi_header)
                                .expect("no avi header for stream"),
                            streams,
                        });
                    }
                    BaseList::MovieList(movi) => {
                        let stream_info =
                            avi_info.as_ref().expect("no stream info for stream data");
                        println!("stream info: {stream_info:?}");
                        let mut movi_iter = movi.entries_async(reader, &opts).unwrap();
                        while let Some(movi) = movi_iter.next().await {
                            let movi = movi.unwrap();
                            println!("movi: {movi:?}");
                        }
                    }
                    _ => (),
                }
            }
            Entry::CharacterSet(cset) => {
                println!("cset: {cset:?}");
            }
            Entry::AviIndex(index) => {
                println!("index: {index:?}");
            }
            _ => (),
        }
    }
    panic!();
}
