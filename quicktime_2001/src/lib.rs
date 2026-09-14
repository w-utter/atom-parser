use atom_parser::FourCC;
use atom_parser::make_atom;

make_atom! {
    #[atom("root")]
    struct Root {
        #[children]
        enum Child {
            FileType,
            Movie,
        }
    }

    #[atom("ftyp")]
    struct FileType {
        major_brand: FourCC,
        minor_version: u32,
        #[trailing_array]
        compatible_brands: FourCC,
    }

    #[atom("moov")]
    struct Movie {
        #[children]
        enum Child {
            MovieHeader,
            Clipping,
            Track,
            Userdata,
            ColorTable,
            CompressedMovie,
            ReferenceMovie,
        }
    }
}

// empty atoms
make_atom! {
    #[atom("wide")]
    struct Wide {}

    #[atom("free")]
    struct Free {
        #[payload]
        free_space: Vec<u8>,
    }

    #[atom("skip")]
    struct Skip {
        #[payload]
        free_space: Vec<u8>,
    }
}

// this can kinda just be anywhere and contain anything
make_atom! {
    #[atom("udta")]
    struct Userdata {

    }
}

// compressed movie data
make_atom! {
    #[atom("cmov")]
    struct CompressedMovie {
        #[children]
        enum Child {
            DataCompression,
            CompressedMovieData,
        }
    }

    #[atom("dcom")]
    struct DataCompression {
        compression_algorithm: u32,
    }

    #[atom("cmvd")]
    struct CompressedMovieData {
        #[payload]
        compressed_movie_data: Vec<u8>,
    }
}

// reference movies
make_atom! {
    #[atom("rmra")]
    struct ReferenceMovie {
        #[trailing_array]
        reference_movie_descriptors: ReferenceMovieDescriptor,
    }

    #[atom("rmda")]
    struct ReferenceMovieDescriptor {
        #[children]
        enum Child {
            DataReference2,
            CPUSpeed,
            VersionCheck,
            ComponentDetect,
            Quality,
        }
    }

    #[atom("rdrf")]
    struct DataReference2 {
        #[flags(u32)]
        flags: struct Flags {
            const SELF_CONTAINED = 1 << 0;
        },
        data_reference: dref::v0::Child,
    }

    #[atom("rmdr")]
    struct DataRate {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        data_rate: u32,
    }

    #[atom("rmcs")]
    struct CPUSpeed {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        cpu_speed: u32,
    }

    #[atom("rmvc")]
    struct VersionCheck {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        software_package: u32,
        version: u32,
        mask: u32,
        check_type: u16,
    }

    #[atom("rmcd")]
    struct ComponentDetect {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        component_description: ComponentDescription,
        minimum_version: u32,
    }

    struct ComponentDescription {
        component_type: FourCC,
        component_subtype: FourCC,
        component_manufacturer: FourCC,
        component_flags: u32,
        component_flags_mask: u32,
    }

    #[atom("rmqu")]
    struct Quality {
        quality: u32,
    }
}

// stuff in movie
make_atom! {
    #[version(0)]
    #[atom("mvhd")]
    struct MovieHeader {
        #[full_box]
        struct Flags {
            // empty
        },
        creation_time: u32,
        modification_time: u32,
        time_scale: u32,
        duration: u32,
        preferred_rate: u32,
        preferred_volume: u16,
        #[reserved]
        reserved: [u8; 10],
        display_matrix: [[u32; 3]; 3],
        preview_time: u32,
        preview_duration: u32,
        poster_time: u32,
        selection_time: u32,
        selection_duration: u32,
        current_time: u32,
        next_track_id: u32,
    }

    #[atom("ctab")]
    struct ColorTable {
        seed: u32,
        #[flags(u16)]
        flags: struct Flags {
            #[expected]
            const EXPECTED = 0x8000;
        },
        #[dynamic_array(size_type = u16, zero_relative = true)]
        color_table: Color,
    }

    struct Color {
        #[reserved]
        reserved: u16,
        red: u16,
        green: u16,
        blue: u16,
    }

    #[atom("trak")]
    struct Track {
        #[children]
        enum Child {
            TrackHeader,
            Clipping,
            TrackMatte,
            Edit,
            TrackReference,
            TrackLoadingSettings,
            TrackInputMap,
            Media,
            Userdata,
        }
    }
}

// clipping region
// can be used in movies/tracks
make_atom! {
    #[atom("clip")]
    struct Clipping {
        #[children]
        enum Child {
            ClippingRegion,
        }
    }

    #[atom("crgn")]
    struct ClippingRegion {
        region_size: u16,
        boundary_box: [u8; 8],
        #[payload]
        clipping_region: Vec<u8>
    }
}

// track specific
make_atom! {
    #[version(0)]
    #[atom("tkhd")]
    struct TrackHeader {
        #[full_box]
        struct Flags {
            const ENABLED = 1 << 0;
            const USED = 1 << 1;
            const USED_IN_PREVIEW = 1 << 2;
            const USED_IN_POSTER = 1 << 3;
        },
        creation_time: u32,
        modification_time: u32,
        track_id: u32,
        #[reserved]
        reserved: [u8; 4],
        duration: u32,
        #[reserved]
        reserved: [u8; 8],
        layer: u16,
        alternate_group: u16,
        volume: u16,
        #[reserved]
        reserved: [u8; 2],
        matrix: [[u32; 3]; 3],
        track_width: u32,
        track_height: u32,
    }


    #[atom("matt")]
    struct TrackMatte {
        #[children]
        enum Child {
            CompressedMatte,
        }
    }
    #[version(0)]
    #[atom("kmat")]
    struct CompressedMatte {
        #[full_box]
        struct Flags {
            // unused
        },
        // TODO:
        // technically any video description can be here
    }

    #[atom("edts")]
    struct Edit {
        #[children]
        enum Child {
            EditList,
        }
    }

    #[version(0)]
    #[atom("elst")]
    struct EditList {
        #[full_box]
        struct Flags {
            // empty
        },
        #[dynamic_array(size_type = u32, zero_relative = false)]
        table_entries: EditListEntry,
    }

    struct EditListEntry {
        duration: u32,
        media_time: u32,
        media_rate: u32,
    }

    #[atom("tref")]
    struct TrackReference {
        #[children]
        enum Child {
            TimeCode,
            ChapterList,
            Synchonization,
            Transcript,
            NonprimarySource,
            Hint,
        }
    }

    #[atom("tmcd")]
    struct TimeCode {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[atom("chap")]
    struct ChapterList {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[atom("sync")]
    struct Synchonization {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[atom("scpt")]
    struct Transcript {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[atom("ssrc")]
    struct NonprimarySource {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[atom("hint")]
    struct Hint {
        #[trailing_array]
        related_track_ids: u32,
    }


    #[atom("load")]
    struct TrackLoadingSettings {
        preload_start_time: u32,
        preload_duration: u32,
        #[flags(u32)]
        preload_flags: struct Flags {
            const PRELOADED_REGARDLESS = 1 << 0;
            const PRELOAD_IF_ENABLED = 1 << 1;
        },
        default_hints: u32,
    }

    #[atom("imap")]
    struct TrackInputMap {
        #[children]
        enum Child {
            TrackInput,
        }
    }

    #[atom(b"\0\0in")]
    struct TrackInput {
        id: u32,
        #[reserved]
        reserved: [u8; 2],
        child_count: u16,
        #[reserved]
        reserved: [u8; 4],
        #[children]
        enum Child {
            InputType,
            ObjectId,
        }
    }

    #[atom(b"\0\0ty")]
    struct InputType {
        ty: u32,
    }

    #[atom("obid")]
    struct ObjectId {
        object_id: u32,
    }

    #[atom("mdia")]
    struct Media {
        #[children]
        enum Child {
            MediaHeader,
            HandlerReference,
            MediaInformation,
            Userdata,
        }
    }
}

//media
make_atom! {
    #[version(0)]
    #[atom("mdhd")]
    struct MediaHeader {
        #[full_box]
        struct Flags {
            // empty
        },
        creation_time: u32,
        modification_time: u32,
        time_scale: u32,
        duration: u32,
        language: u16,
        quality: u16,
    }

    #[version(0)]
    #[atom("hdlr")]
    struct HandlerReference {
        #[full_box]
        struct Flags {
            // empty
        },
        component_type: u32,
        component_subtype: u32,
        #[reserved]
        component_manufacturer: u32,
        #[reserved]
        component_flags: u32,
        #[reserved]
        component_flags_mask: u32,
        #[pascal_string(u8)]
        component_name: String,
    }
    #[atom("minf")]
    struct MediaInformation {
        #[children]
        enum Child {
            VideoMediaInformationHeader,
            SoundMediaInformationHeader,
            TimecodeMediaInformation,
            BaseMediaInformationHeader,
            BaseMediaInformation,
            HandlerReference,
            DataInformation,
            SampleTable,
        }
    }
}

// media information
make_atom! {
    #[version(0)]
    #[atom("vmhd")]
    struct VideoMediaInformationHeader {
        #[full_box]
        struct Flags {
            const NO_LEAN_AHEAD = 1 << 0;
        },
        graphics_mode: u16,
        opcolor: [u16; 3],
    }

    #[version(0)]
    #[atom("smhd")]
    struct SoundMediaInformationHeader {
        #[full_box]
        struct Flags {
            // empty
        },
        balance: u16,
        #[reserved]
        resered: [u8; 2]
    }

    #[atom("gmhd")]
    struct BaseMediaInformationHeader {
        // empty...
    }

    #[version(0)]
    #[atom("gmin")]
    struct BaseMediaInformation {
        #[full_box]
        struct Flags {
            // empty
        },
        graphics_mode: u16,
        opcolor: [u16; 3],
        balance: u16,
        #[reserved]
        reserved: [u8; 2]
    }

    #[version(0)]
    #[atom("tmci")]
    struct TimecodeMediaInformation {
        #[full_box]
        struct Flags {
            // empty
        },
        text_font: u16,
        #[flags(u16)]
        text_face: struct TextFace {
            const BOLD = 1 << 0;
            const ITALIC = 1 << 1;
            const UNDERLINE = 1 << 2;
            const OUTLINE = 1 << 3;
            const SHADOW = 1 << 4;
            const CONDENSE = 1 << 5;
            const EXTEND = 1 << 6;
        },
        text_size: u16,
        text_color: [u16; 3],
        background_color: [u16; 3],
        #[pascal_string(u8)]
        font_name: String,
    }
}

// sample table
make_atom! {
    #[atom("stbl")]
    struct SampleTable {
        #[children]
        enum Child {
            SampleDescription,
            TimeToSample,
            SyncSample,
            SampleToChunk,
            SampleSize,
            ChunkOffset,
            // ShadowSync, reserved
        },
    }

    struct SampleDescriptionEntry<D> {
        #[reserved]
        reserved: [u8; 6],
        data_reference_index: u16,
        description: D,
    }

    #[version(0)]
    #[atom("stsd")]
    struct SampleDescription {
        #[full_box]
        struct Flags {
            // empty
        },
        #[children(u32)]
        enum Child {
            // TODO
        }
    }

    #[version(0)]
    #[atom("stts")]
    struct TimeToSample {
        #[full_box]
        struct Flags {
            // empty
        },
        #[dynamic_array(size_type = u32, zero_relative = false)]
        time_to_sample_table: TimeToSampleTableEntry,
    }

    struct TimeToSampleTableEntry {
        sample_count: u32,
        sample_duration: u32,
    }

    #[version(0)]
    #[atom("stss")]
    struct SyncSample {
        #[full_box]
        struct Flags {
            // empty
        },
        #[dynamic_array(size_type = u32, zero_relative = false)]
        sync_sample_table: u32,
    }

    #[version(0)]
    #[atom("stsc")]
    struct SampleToChunk {
        #[full_box]
        struct Flags {
            // empty
        },
        #[dynamic_array(size_type = u32, zero_relative = false)]
        sample_to_chunk_table: SampleToChunkTableEntry,
    }

    struct SampleToChunkTableEntry {
        first_chunk: u32,
        samples_per_chunk: u32,
        sample_description_id: u32,
    }

    #[version(0)]
    #[atom("stsz")]
    struct SampleSize {
        #[full_box]
        struct Flags {
            // empty
        },
        sample_size: u32,
        #[dynamic_array(size_type = u32, zero_relative = false)]
        sample_size_table: u32,
    }
}

// data information
make_atom! {
    #[atom("dinf")]
    struct DataInformation {
        #[children]
        enum Child {
            DataReference,
        }
    }

    #[version(0)]
    #[atom("dref")]
    struct DataReference {
        #[full_box]
        struct Flags {
            // empty
        },
        #[children(u32)]
        enum Child {
            MacAlias,
            MacResource,
            Url,
        }
    }

    #[version(0)]
    #[atom("alis")]
    struct MacAlias {
        #[full_box]
        struct Flags {
            const SELF_REFERENTIAL = 1 << 0;
        },
        #[payload]
        mac_alias: Vec<u8>,
    }

    #[version(0)]
    #[atom("rsrc")]
    struct MacResource {
        #[full_box]
        struct Flags {
            const SELF_REFERENTIAL = 1 << 0;
        },
        #[payload]
        mac_alias_resource: Vec<u8>,
    }

    #[version(0)]
    #[atom(b"url\0")]
    struct Url {
        #[full_box]
        struct Flags {
            const SELF_REFERENTIAL = 1 << 0;
        },
        #[payload]
        url: Vec<u8>,
    }

    #[version(0)]
    #[atom("stco")]
    struct ChunkOffset {
        #[full_box]
        struct Flags {
            // empty
        },
        #[dynamic_array(size_type = u32, zero_relative = false)]
        chunk_offset_table: u32,
    }
}

// video media
pub mod sample_description {
    use super::*;
    make_atom! {
        struct Video {
            version: u16,
            #[reserved]
            revision_level: u16,
            vendor: u32,
            temporal_quality: u32,
            spatial_quality: u32,
            width: u16,
            height: u16,
            horizontal_resolution: u32,
            vertical_resolution: u32,
            #[reserved]
            data_size: u32,
            // frames of data per sample
            frame_count: u16,
            #[pascal_string(u32)]
            compressor_name: String,
            pixel_depth: u32,
            color_table_id: u16,
        }
        // TODO: mjpeg stuff?
        // - see page 99


        struct Sound {
            version: u16,
            revision_level: u16,
            vendor: u32,
            number_of_channels: u16,
            sample_size: u16,
            compression_id: u16,
            packet_size: u16,
            sample_rate: u32,
        }

        struct Timecode {
            #[reserved]
            reserved: u32,
            #[flags(u32)]
            flags: struct Flags {
                const DROP_FRAME = 1 << 0;
                const WRAP_AFTER_24H = 1 << 1;
                const SUPPORTS_NEGATIVE_TIME = 1 << 2;
                const IS_TAPE_COUNTER = 1 << 3;
            },
            time_scale: u32,
            frame_duration: u32,
            number_of_frames: u8,
            #[reserved]
            reserved: [u8; 3],
            source_reference: Userdata,
        }

        struct Text {
            #[flags(u32)]
            display_flags: struct DisplayFlags {
                const DONT_AUTO_SCALE = 1 << 1;
                const USE_MOVIE_BACKGROUND_COLOR = 1 << 4;
                const SCROLL_IN = 1 << 5;
                const SCROLL_OUT = 1 << 6;
                const HORIZONTAL_SCROLL = 1 << 7;
                const REVERSE_SCROLL = 1 << 8;
                const CONTINUOUS_SCROLL = 1 << 9;
                const DROP_SHADOW = 1 << 12;
                const ANTI_ALIAS = 1 << 13;
                const KEY_TEXT = 1 << 14;
            },
            text_justification: u32,
            default_text_box: u64,
            #[reserved]
            reserved: [u8; 8],
            font_number: u16,
            #[flags(u16)]
            font_face: struct FontFace {
                const BOLD = 1 << 0;
                const ITALIC = 1 << 1;
                const UNDERLINE = 1 << 2;
                const OUTLINE = 1 << 3;
                const SHADOW = 1 << 4;
                const CONDENSE = 1 << 5;
                const EXTEND = 1 << 6;
            },
            #[reserved]
            reserved: u8,
            #[reserved]
            reserved: [u8; 2],
            foreground_color: [u16; 3],
            #[pascal_string(u8)]
            text_name: String,
        }
        // TODO: text sample extensions / hypertext
        // - see page 111

        struct Music {
            #[flags(u32)]
            flags: struct Flags {
                // empty
            }
        }

        struct Mpeg {
            // empty
        }

        struct Sprite {
            // empty
        }

        struct Tween {
            // empty
        }

        struct Q3D {
            // empty
        }

        struct Streaming {
            version: u32,
            #[reserved]
            reserved: [u8; 4],
            flags: u32,
        }

        struct Hint {
            version: u16,
            last_compatible_version: u16,
            max_packet_size: u32,
            // TODO: children for rtp
            // - see page 151
        }
    }
}

pub type VideoSampleDescription = SampleDescriptionEntry<sample_description::Video>;
pub type SoundSampleDescription = SampleDescriptionEntry<sample_description::Sound>;
pub type TimecodeSampleDescription = SampleDescriptionEntry<sample_description::Timecode>;
pub type TextSampleDescription = SampleDescriptionEntry<sample_description::Text>;
pub type MusicSampleDescription = SampleDescriptionEntry<sample_description::Music>;
pub type MpegSampleDescription = SampleDescriptionEntry<sample_description::Mpeg>;
pub type SpriteSampleDescription = SampleDescriptionEntry<sample_description::Sprite>;
pub type TweenSampleDescription = SampleDescriptionEntry<sample_description::Sprite>;
pub type Q3DSampleDescription = SampleDescriptionEntry<sample_description::Q3D>;
pub type StreamingSampleDescription = SampleDescriptionEntry<sample_description::Streaming>;

// hints
make_atom! {
    #[atom("hnti")]
    struct HintInfo {
        #[children]
        enum Child {

        }
    }

    #[atom("trpy")]
    struct TrackPayloadSizeHint64 {
        byte_len: u64,
    }
    #[atom("totl")]
    struct TrackPayloadSizeHint32 {
        byte_len: u32,
    }
    #[atom("nump")]
    struct NetworkPacketHint64 {
        network_packet_count: u64,
    }
    #[atom("npck")]
    struct NetworkPacketHint32 {
        network_packet_count: u32,
    }
    #[atom("tpyl")]
    struct ByteCountHint64 {
        total_bytes_minus_rtp_headers: u64,
    }
    #[atom("tpay")]
    struct ByteCountHint32 {
        total_bytes_minus_rtp_headers: u32,
    }
    #[atom("maxr")]
    struct DatarateHint {
        granularity_ms: u32,
        maximum_datarate: u32,
    }
    #[atom("dmed")]
    struct MediaTrackBytesHint {
        byte_count: u64,
    }
    #[atom("dimm")]
    struct ImmediateBytesHint {
        byte_count: u64,
    }
    #[atom("drep")]
    struct RepeatedBytesHint {
        byte_count: u64,
    }
    #[atom("tmin")]
    struct MinTransmissionTimeHint {
        shortest_transmission_ms: u32,
    }
    #[atom("tmax")]
    struct MaxTransmissionTimeHint {
        longest_transmission_ms: u32,
    }
    #[atom("pmax")]
    struct LargestPacketHint {
        byte_count: u32,
    }
    #[atom("dmax")]
    struct LargestPacketDurationHint {
        largest_duration_ms: u32,
    }
    #[atom("payt")]
    struct PayloadTypeHint {
        payload_number: u32,
        #[pascal_string(u8)]
        rtpmap: String,
    }
}

#[test]
fn ftyp() {
    let mut r =
        atom_parser::InMemoryReader::from_path("../file_example_MOV_480_700kB.mov").unwrap();
    let opts = Default::default();

    use atom_parser::Parse;

    let root = Root::parse(&mut r, &opts).unwrap();
    let mut root_iter = root.children(&mut r, &opts);

    while let Some(child) = root_iter.next() {
        let r = &mut root_iter.reader;
        match child.unwrap() {
            root::Child::FileType(ftyp) => println!("file type: {ftyp:?}"),
            root::Child::Movie(movie) => {
                println!("moov: {movie:?}");
                let mut child_iter = movie.children(r, &opts);

                while let Some(child) = child_iter.next() {
                    let r = &mut child_iter.reader;
                    match child.unwrap() {
                        moov::Child::MovieHeader(hd) => {
                            println!("header: {hd:?}");
                        }
                        moov::Child::Clipping(clip) => {
                            println!("clip: {clip:?}");
                        }
                        moov::Child::Track(track) => {
                            println!("track: {track:?}");
                            let mut child_iter = track.children(r, &opts);
                            while let Some(child) = child_iter.next() {
                                let r = &mut child_iter.reader;
                                match child.unwrap() {
                                    trak::Child::TrackHeader(hdr) => {
                                        println!("track header: {hdr:?}")
                                    }
                                    trak::Child::Clipping(c) => println!("clipping: {c:?}"),
                                    trak::Child::TrackMatte(tm) => println!("track matte: {tm:?}"),
                                    trak::Child::Edit(e) => {
                                        println!("edit: {e:?}");
                                        let mut child_iter = e.children(r, &opts);
                                        while let Some(child) = child_iter.next() {
                                            let r = &mut child_iter.reader;
                                            match child.unwrap() {
                                                edts::Child::EditList(el) => match el.version {
                                                    elst::EditListVersions::V0(el) => {
                                                        println!("edit list: {el:?}");
                                                        let list_entires = el
                                                            .table_entries(r, &opts)
                                                            .collect::<Result<Vec<_>, _>>()
                                                            .unwrap();
                                                        println!(
                                                            "edit list entries: {list_entires:?}"
                                                        );
                                                    }
                                                    elst::EditListVersions::Unknown(v) => {
                                                        println!("unknown elst: {v}")
                                                    }
                                                },
                                                _ => (),
                                            }
                                        }
                                    }
                                    trak::Child::TrackReference(tref) => {
                                        println!("track ref: {tref:?}")
                                    }
                                    trak::Child::TrackLoadingSettings(tls) => {
                                        println!("track loading: {tls:?}")
                                    }
                                    trak::Child::TrackInputMap(tim) => {
                                        println!("track input map: {tim:?}")
                                    }
                                    trak::Child::Media(m) => {
                                        let mut child_iter = m.children(r, &opts);
                                        while let Some(child) = child_iter.next() {
                                            let r = &mut child_iter.reader;
                                            match child.unwrap() {
                                                mdia::Child::MediaHeader(h) => {
                                                    println!("media heaader: {h:?}")
                                                }
                                                mdia::Child::HandlerReference(r) => {
                                                    println!("href: {r:?}")
                                                }
                                                mdia::Child::MediaInformation(info) => {
                                                    println!("info: {info:?}");
                                                    let mut child_iter = info.children(r, &opts);
                                                    while let Some(child) = child_iter.next() {
                                                        let r = &mut child_iter.reader;
                                                        match child.unwrap() {
                                                            minf::Child::VideoMediaInformationHeader(vid) => println!("vid: {vid:?}"),
                                                            minf::Child::SoundMediaInformationHeader(snd) => println!("snd: {snd:?}"),
                                                            minf::Child::TimecodeMediaInformation(info) => println!("timecode: {info:?}"),
                                                            minf::Child::BaseMediaInformationHeader(base) => println!("base: {base:?}"),
                                                            minf::Child::BaseMediaInformation(base) => println!("base info: {base:?}"),
                                                            minf::Child::HandlerReference(href) => println!("href: {href:?}"),
                                                            minf::Child::DataInformation(dinfo) => {
                                                                println!("dinfo: {dinfo:?}");
                                                                let mut child_iter = dinfo.children(r, &opts);
                                                                while let Some(child) = child_iter.next() {
                                                                    let r = &mut child_iter.reader;
                                                                    match child.unwrap() {
                                                                        dinf::Child::DataReference(dref) => {
                                                                            match dref.version {
                                                                                dref::DataReferenceVersions::V0(dref) => {
                                                                                    let mut child_iter = dref.children(r, &opts);
                                                                                    while let Some(child) = child_iter.next() {
                                                                                        match child.unwrap() {
                                                                                            dref::v0::Child::MacAlias(alis) => println!("alias: {alis:?}"),
                                                                                            dref::v0::Child::MacResource(rsrc) => println!("r: {rsrc:?}"),
                                                                                            dref::v0::Child::Url(url) => println!("url: {url:?}"),
                                                                                            _ => (),
                                                                                        }
                                                                                    }
                                                                                }
                                                                                dref::DataReferenceVersions::Unknown(v) => println!("unknown dref: {v}"),
                                                                            }
                                                                        }
                                                                        _ => (),
                                                                    }
                                                                }
                                                            }
                                                            minf::Child::SampleTable(stable) => {
                                                                println!("stable: {stable:?}");
                                                                let mut child_iter = stable.children(r, &opts);
                                                                while let Some(child) = child_iter.next() {
                                                                    let r = &mut child_iter.reader;
                                                                    match child.unwrap() {
                                                                        stbl::Child::SampleDescription(desc) => {
                                                                            match desc.version {
                                                                                stsd::SampleDescriptionVersions::V0(desc) => {
                                                                                    println!("{desc:?}");
                                                                                    let mut child_iter = desc.children(r, &opts);
                                                                                    while let Some(child) = child_iter.next() {
                                                                                        let _r = &mut child_iter.reader;
                                                                                        println!("sample desc: {child:?}");
                                                                                    }
                                                                                }
                                                                                stsd::SampleDescriptionVersions::Unknown(v) => println!("unknown stsd: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::TimeToSample(tts) => {
                                                                            match tts.version {
                                                                                stts::TimeToSampleVersions::V0(tts) => {
                                                                                    println!("tts: {tts:?}");
                                                                                    let time_to_sample = tts.time_to_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                    println!("tts entries: {time_to_sample:?}");
                                                                                }
                                                                                stts::TimeToSampleVersions::Unknown(v) => println!("unknown stts: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::SyncSample(sync) => {
                                                                            match sync.version {
                                                                                stss::SyncSampleVersions::V0(sync) => {
                                                                                    println!("sync sample: {sync:?}");
                                                                                    let sync_samples = sync.sync_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                    println!("sync sample entries: {sync_samples:?}");
                                                                                }
                                                                                stss::SyncSampleVersions::Unknown(v) => println!("unknown stts: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::SampleToChunk(stc) => {
                                                                            match stc.version {
                                                                                stsc::SampleToChunkVersions::V0(stc) => {
                                                                                    println!("stc: {stc:?}");
                                                                                    let sample_to_chunk = stc.sample_to_chunk_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                    println!("stc entries: {sample_to_chunk:?}");
                                                                                }
                                                                                stsc::SampleToChunkVersions::Unknown(v) => println!("unknown stsc: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::SampleSize(ss) => {
                                                                            match ss.version {
                                                                                stsz::SampleSizeVersions::V0(ss) => {
                                                                                    println!("ss: {ss:?}");
                                                                                    let sample_sizes = ss.sample_size_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                    println!("ss entries: {sample_sizes:?}");
                                                                                }
                                                                                stsz::SampleSizeVersions::Unknown(v) => println!("unknown stsz: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::ChunkOffset(co) => {
                                                                            match co.version {
                                                                                stco::ChunkOffsetVersions::V0(co) => {
                                                                                    println!("co32: {co:?}");
                                                                                    let offsets = co.chunk_offset_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                    println!("co32 entries: {offsets:?}")
                                                                                }
                                                                                stco::ChunkOffsetVersions::Unknown(v) => println!("unknown stco: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::Unsupported(u) => println!("unsupported stbl entry: {u:?}"),
                                                                    }
                                                                }
                                                            }
                                                            minf::Child::Unsupported(_) => (),
                                                        }
                                                    }
                                                }
                                                mdia::Child::Userdata(u) => {
                                                    println!("udata: {u:?}")
                                                }
                                                mdia::Child::Unsupported(_) => (),
                                            }
                                        }
                                    }
                                    trak::Child::Userdata(udata) => println!("udata: {udata:?}"),
                                    trak::Child::Unsupported(fcc) => {
                                        println!("unsupported in trak: {fcc:?}")
                                    }
                                }
                            }
                        }
                        moov::Child::Userdata(udta) => {
                            println!("udata: {udta:?}");
                        }
                        moov::Child::ColorTable(ctb) => {
                            println!("color table: {ctb:?}");
                        }
                        moov::Child::Unsupported(missed) => {
                            println!("skipped: {missed:?}");
                        }
                        _ => (),
                    }
                }
            }
            root::Child::Unsupported(a) => println!("unsupported {a:?}"),
        }
    }
    panic!()
}

#[tokio::test]
async fn ftyp_async() {
    let mut r =
        atom_parser::InMemoryReader::from_path("../file_example_MOV_480_700kB.mov").unwrap();
    let opts = Default::default();

    use atom_parser::AsyncParse;

    let root = Root::parse_async(&mut r, &opts).await.unwrap();
    println!("{root:?}");
    let mut root_iter = root.children_async(&mut r, &opts);
    use tokio_stream::StreamExt;
    while let Some(child) = root_iter.next().await {
        let r = root_iter.reader();
        match child.unwrap() {
            root::Child::FileType(ftyp) => println!("file type: {ftyp:?}"),
            root::Child::Movie(movie) => {
                println!("moov: {movie:?}");
                let mut child_iter = movie.children_async(r, &opts);

                while let Some(child) = child_iter.next().await {
                    let r = child_iter.reader();
                    match child.unwrap() {
                        moov::Child::MovieHeader(hd) => {
                            println!("header: {hd:?}");
                        }
                        moov::Child::Clipping(clip) => {
                            println!("clip: {clip:?}");
                        }
                        moov::Child::Track(track) => {
                            println!("track: {track:?}");
                            let mut child_iter = track.children_async(r, &opts);
                            while let Some(child) = child_iter.next().await {
                                let r = child_iter.reader();
                                match child.unwrap() {
                                    trak::Child::TrackHeader(hdr) => {
                                        println!("track header: {hdr:?}")
                                    }
                                    trak::Child::Clipping(c) => println!("clipping: {c:?}"),
                                    trak::Child::TrackMatte(tm) => println!("track matte: {tm:?}"),
                                    trak::Child::Edit(e) => {
                                        println!("edit: {e:?}");
                                        let mut child_iter = e.children_async(r, &opts);
                                        while let Some(child) = child_iter.next().await {
                                            let r = child_iter.reader();
                                            match child.unwrap() {
                                                edts::Child::EditList(el) => match el.version {
                                                    elst::EditListVersions::V0(el) => {
                                                        println!("edit list: {el:?}");
                                                        let list_entires = el
                                                            .table_entries_async(r, &opts)
                                                            .collect::<Result<Vec<_>, _>>()
                                                            .await
                                                            .unwrap();
                                                        println!(
                                                            "edit list entries: {list_entires:?}"
                                                        );
                                                    }
                                                    elst::EditListVersions::Unknown(v) => {
                                                        println!("unknown elst: {v}")
                                                    }
                                                },
                                                _ => (),
                                            }
                                        }
                                    }
                                    trak::Child::TrackReference(tref) => {
                                        println!("track ref: {tref:?}")
                                    }
                                    trak::Child::TrackLoadingSettings(tls) => {
                                        println!("track loading: {tls:?}")
                                    }
                                    trak::Child::TrackInputMap(tim) => {
                                        println!("track input map: {tim:?}")
                                    }
                                    trak::Child::Media(m) => {
                                        let mut child_iter = m.children_async(r, &opts);
                                        while let Some(child) = child_iter.next().await {
                                            let r = child_iter.reader();
                                            match child.unwrap() {
                                                mdia::Child::MediaHeader(h) => {
                                                    println!("media heaader: {h:?}")
                                                }
                                                mdia::Child::HandlerReference(r) => {
                                                    println!("href: {r:?}")
                                                }
                                                mdia::Child::MediaInformation(info) => {
                                                    println!("info: {info:?}");
                                                    let mut child_iter =
                                                        info.children_async(r, &opts);
                                                    while let Some(child) = child_iter.next().await
                                                    {
                                                        let r = child_iter.reader();
                                                        match child.unwrap() {
                                                            minf::Child::VideoMediaInformationHeader(vid) => println!("vid: {vid:?}"),
                                                            minf::Child::SoundMediaInformationHeader(snd) => println!("snd: {snd:?}"),
                                                            minf::Child::TimecodeMediaInformation(info) => println!("timecode: {info:?}"),
                                                            minf::Child::BaseMediaInformationHeader(base) => println!("base: {base:?}"),
                                                            minf::Child::BaseMediaInformation(base) => println!("base info: {base:?}"),
                                                            minf::Child::HandlerReference(href) => println!("href: {href:?}"),
                                                            minf::Child::DataInformation(dinfo) => {
                                                                println!("dinfo: {dinfo:?}");
                                                                let mut child_iter = dinfo.children_async(r, &opts);
                                                                while let Some(child) = child_iter.next().await {
                                                                    let r = child_iter.reader();
                                                                    match child.unwrap() {
                                                                        dinf::Child::DataReference(dref) => {
                                                                            match dref.version {
                                                                                dref::DataReferenceVersions::V0(dref) => {
                                                                                    let mut child_iter = dref.children_async(r, &opts);
                                                                                    while let Some(child) = child_iter.next().await {
                                                                                        match child.unwrap() {
                                                                                            dref::v0::Child::MacAlias(alis) => println!("alias: {alis:?}"),
                                                                                            dref::v0::Child::MacResource(rsrc) => println!("r: {rsrc:?}"),
                                                                                            dref::v0::Child::Url(url) => println!("url: {url:?}"),
                                                                                            _ => (),
                                                                                        }
                                                                                    }
                                                                                }
                                                                                dref::DataReferenceVersions::Unknown(v) => println!("unknown dref: {v}"),
                                                                            }
                                                                        }
                                                                        _ => (),
                                                                    }
                                                                }
                                                            }
                                                            minf::Child::SampleTable(stable) => {
                                                                println!("stable: {stable:?}");
                                                                let mut child_iter = stable.children_async(r, &opts);
                                                                while let Some(child) = child_iter.next().await {
                                                                    let r = child_iter.reader();
                                                                    match child.unwrap() {
                                                                        stbl::Child::SampleDescription(desc) => {
                                                                            match desc.version {
                                                                                stsd::SampleDescriptionVersions::V0(desc) => {
                                                                                    println!("{desc:?}");
                                                                                    let mut child_iter = desc.children_async(r, &opts);
                                                                                    while let Some(child) = child_iter.next().await {
                                                                                        let _r = child_iter.reader();
                                                                                        println!("sample desc: {child:?}");
                                                                                    }
                                                                                }
                                                                                stsd::SampleDescriptionVersions::Unknown(v) => println!("unknown stsd: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::TimeToSample(tts) => {
                                                                            match tts.version {
                                                                                stts::TimeToSampleVersions::V0(tts) => {
                                                                                    println!("tts: {tts:?}");
                                                                                    let time_to_sample = tts.time_to_sample_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                                    println!("tts entries: {time_to_sample:?}");
                                                                                }
                                                                                stts::TimeToSampleVersions::Unknown(v) => println!("unknown stts: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::SyncSample(sync) => {
                                                                            match sync.version {
                                                                                stss::SyncSampleVersions::V0(sync) => {
                                                                                    println!("sync sample: {sync:?}");
                                                                                    let sync_samples = sync.sync_sample_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                                    println!("sync sample entries: {sync_samples:?}");
                                                                                }
                                                                                stss::SyncSampleVersions::Unknown(v) => println!("unknown stts: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::SampleToChunk(stc) => {
                                                                            match stc.version {
                                                                                stsc::SampleToChunkVersions::V0(stc) => {
                                                                                    println!("stc: {stc:?}");
                                                                                    let sample_to_chunk = stc.sample_to_chunk_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                                    println!("stc entries: {sample_to_chunk:?}");
                                                                                }
                                                                                stsc::SampleToChunkVersions::Unknown(v) => println!("unknown stsc: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::SampleSize(ss) => {
                                                                            match ss.version {
                                                                                stsz::SampleSizeVersions::V0(ss) => {
                                                                                    println!("ss: {ss:?}");
                                                                                    let sample_sizes = ss.sample_size_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                                    println!("ss entries: {sample_sizes:?}");
                                                                                }
                                                                                stsz::SampleSizeVersions::Unknown(v) => println!("unknown stsz: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::ChunkOffset(co) => {
                                                                            match co.version {
                                                                                stco::ChunkOffsetVersions::V0(co) => {
                                                                                    println!("co32: {co:?}");
                                                                                    let offsets = co.chunk_offset_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                                    println!("co32 entries: {offsets:?}")
                                                                                }
                                                                                stco::ChunkOffsetVersions::Unknown(v) => println!("unknown stco: {v}"),
                                                                            }
                                                                        }
                                                                        stbl::Child::Unsupported(u) => println!("unsupported stbl entry: {u:?}"),
                                                                    }
                                                                }
                                                            }
                                                            minf::Child::Unsupported(_) => (),
                                                        }
                                                    }
                                                }
                                                mdia::Child::Userdata(u) => {
                                                    println!("udata: {u:?}")
                                                }
                                                mdia::Child::Unsupported(_) => (),
                                            }
                                        }
                                    }
                                    trak::Child::Userdata(udata) => println!("udata: {udata:?}"),
                                    trak::Child::Unsupported(fcc) => {
                                        println!("unsupported in trak: {fcc:?}")
                                    }
                                }
                            }
                        }
                        moov::Child::Userdata(udta) => {
                            println!("udata: {udta:?}");
                        }
                        moov::Child::ColorTable(ctb) => {
                            println!("color table: {ctb:?}");
                        }
                        moov::Child::Unsupported(missed) => {
                            println!("skipped: {missed:?}");
                        }
                        _ => (),
                    }
                }
            }
            root::Child::Unsupported(a) => println!("unsupported {a:?}"),
        }
    }
    panic!()
}
