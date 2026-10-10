use atom_parser::FourCC;
use atom_parser::make_atom;

make_atom! {
    #[derive(Debug)]
    #[atom("root")]
    struct Root {
        #[children]
        enum Child {
            FileType,
            Movie,
            MovieData,
        }
    }

    #[derive(Debug)]
    #[atom("ftyp")]
    struct FileType {
        major_brand: FourCC,
        minor_version: u32,
        #[trailing_array]
        compatible_brands: FourCC,
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("mdat")]
    struct MovieData {
        #[payload]
        data: Vec<u8>,
    }
}

impl<O: atom_parser::Offset> MovieData<O> {
    pub fn from_offset_32(
        &self,
        extent: atom_parser::Extent<u32, u32>,
    ) -> Result<atom_parser::Extent<O, O>, atom_parser::ParseError> {
        use atom_parser::{ParseError, TryFromIntError};
        let payload = self.data.extent();

        let end: O = (extent.offset + extent.len)
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        if end > payload.offset + payload.len {
            return Err(ParseError::Io(atom_parser::IoError::UnexpectedEndOfData));
        }

        Ok(atom_parser::Extent {
            offset: extent
                .offset
                .try_into()
                .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?,
            len: extent
                .len
                .try_into()
                .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?,
        })
    }
}

// empty atoms
make_atom! {
    #[derive(Debug)]
    #[atom("wide")]
    struct Wide {}

    #[derive(Debug)]
    #[atom("free")]
    struct Free {
        #[payload]
        free_space: Vec<u8>,
    }

    #[derive(Debug)]
    #[atom("skip")]
    struct Skip {
        #[payload]
        free_space: Vec<u8>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[atom("udta")]
    struct Userdata {
        #[children]
        enum Child {
            ArrangerName,
            ArrangerKeywords,
            ComposerKeywords,
            ComposerName,
            CopyrightStatement,
            MovieCreationDate,
            DirectorName,
            Edit1,
            Edit2,
            Edit3,
            Edit4,
            Edit5,
            Edit6,
            Edit7,
            Edit8,
            Edit9,
            MovieFormat,
            MovieInformation,
            ISRCCode,
            RecordLabelName,
            RecordLabelUrl,
            CreatorName,
            CreatorUrl,
            TitleKeywords,
            Title,
            ProducerKeywords,
            RecordingCopyrightStatement,
            ProducerName,
            PerformersNames,
            ArtistAndPerformerKeywords,
            ArtistAndPerformerUrl,
            HardwareAndSoftwareRequirements,
            SubtitleKeywords,
            Subtitle,
            SourceCredits,
            SongwriterName,
            SongwriterKeywords,
            MovieVersionNumber,
            WriterName,
            PlayAllFrames,
            HintTrackStatistics,
            MovieHintInfo,
            Name,
            Loop,
            PrintToVideo,
            PlaySelectionOnly,
            DefaultWindowLocation,
        }
    }
}

// userdata
make_atom! {
    #[derive(Debug)]
    struct StringEntry {
        #[payload_len]
        len: u16,
        language_code: u16,
        #[payload]
        string: String,
    }

    // all © codepoints are replaced with \xA9
    #[derive(Debug)]
    #[atom(b"\xA9arg")]
    struct ArrangerName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ark")]
    struct ArrangerKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9cok")]
    struct ComposerKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9com")]
    struct ComposerName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9cpy")]
    struct CopyrightStatement {
        #[trailing_array]
        statement: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9day")]
    struct MovieCreationDate {
        #[trailing_array]
        date: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9dir")]
    struct DirectorName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed1")]
    struct Edit1 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed2")]
    struct Edit2 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed3")]
    struct Edit3 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed4")]
    struct Edit4 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed5")]
    struct Edit5 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed6")]
    struct Edit6 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed7")]
    struct Edit7 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed8")]
    struct Edit8 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9ed9")]
    struct Edit9 {
        #[trailing_array]
        edit_and_description: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9fmt")]
    struct MovieFormat {
        #[trailing_array]
        format: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9inf")]
    struct MovieInformation {
        #[trailing_array]
        info: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9isr")]
    struct ISRCCode {
        #[trailing_array]
        isrc: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9lab")]
    struct RecordLabelName {
        #[trailing_array]
        record_label: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9lal")]
    struct RecordLabelUrl {
        #[trailing_array]
        url: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9mak")]
    struct CreatorName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9mal")]
    struct CreatorUrl {
        #[trailing_array]
        url: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9nak")]
    struct TitleKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9nam")]
    struct Title {
        #[trailing_array]
        title: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9pdk")]
    struct ProducerKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9phg")]
    struct RecordingCopyrightStatement {
        #[trailing_array]
        statement: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9prd")]
    struct ProducerName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9prf")]
    struct PerformersNames {
        #[trailing_array]
        names: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9prk")]
    struct ArtistAndPerformerKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9prl")]
    struct ArtistAndPerformerUrl {
        #[trailing_array]
        url: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9req")]
    struct HardwareAndSoftwareRequirements {
        #[trailing_array]
        requirements: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9snk")]
    struct SubtitleKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9snm")]
    struct Subtitle {
        #[trailing_array]
        subtitle: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9src")]
    struct SourceCredits {
        #[trailing_array]
        credits: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9swf")]
    struct SongwriterName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9swk")]
    struct SongwriterKeywords {
        #[trailing_array]
        keywords: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9swr")]
    struct MovieVersionNumber {
        #[trailing_array]
        name_and_version: StringEntry,
    }

    #[derive(Debug)]
    #[atom(b"\xA9wrt")]
    struct WriterName {
        #[trailing_array]
        name: StringEntry,
    }

    #[derive(Debug)]
    #[atom("AllF")]
    struct PlayAllFrames {
        play_all: bool,
    }

    #[derive(Debug)]
    #[atom("name")]
    struct Name {
        #[payload]
        object_name: String,
    }

    #[derive(Debug)]
    #[enum_repr(u32)]
    enum LoopKind {
        Normal = 0,
        Palindromic = 1,
    }

    #[derive(Debug)]
    #[atom("LOOP")]
    struct Loop {
        kind: LoopKind,
    }

    #[derive(Debug)]
    #[enum_repr(u16)]
    // NOTE: values are stored in little endian
    // so each value (1-4) is shifted over 8 bits
    enum DisplaySize {
        Normal = 0,
        Double = 0x100,
        Half = 0x200,
        ScaleToFillScreen = 0x300,
        CurrentSize = 0x400,
    }

    #[derive(Debug)]
    #[atom("ptv ")]
    struct PrintToVideo {
        display_size: DisplaySize,
        #[reserved]
        reserved: [u8; 4],
        slideshow: bool,
        play_on_open: bool,
    }

    #[derive(Debug)]
    #[atom("SelO")]
    struct PlaySelectionOnly {
        selection_only: bool
    }

    #[derive(Debug)]
    #[atom("WLOC")]
    struct DefaultWindowLocation {
        x: u16,
        y: u16,
    }
}

// compressed movie data
make_atom! {
    #[derive(Debug)]
    #[atom("cmov")]
    struct CompressedMovie {
        #[children]
        enum Child {
            DataCompression,
            CompressedMovieData,
        }
    }

    #[derive(Debug)]
    #[atom("dcom")]
    struct DataCompression {
        compression_algorithm: FourCC,
    }

    #[derive(Debug)]
    #[atom("cmvd")]
    struct CompressedMovieData {
        #[payload]
        compressed_movie_data: Vec<u8>,
    }
}

// reference movies
make_atom! {
    #[derive(Debug)]
    #[atom("rmra")]
    struct ReferenceMovie {
        #[trailing_array]
        reference_movie_descriptors: ReferenceMovieDescriptor,
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("rdrf")]
    struct DataReference2 {
        #[flags(u32)]
        flags: struct Flags {
            const SELF_CONTAINED = 1 << 0;
        },
        data_reference: dref::v0::Child,
    }

    #[derive(Debug)]
    #[atom("rmdr")]
    struct DataRate {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        data_rate: u32,
    }

    #[derive(Debug)]
    #[atom("rmcs")]
    struct CPUSpeed {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        cpu_speed: u32,
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("rmcd")]
    struct ComponentDetect {
        #[flags(u32)]
        flags: struct Flags {
            // empty
        },
        component_description: ComponentDescription,
        minimum_version: u32,
    }

    #[derive(Debug)]
    struct ComponentDescription {
        component_type: FourCC,
        component_subtype: FourCC,
        component_manufacturer: FourCC,
        component_flags: u32,
        component_flags_mask: u32,
    }

    #[derive(Debug)]
    #[atom("rmqu")]
    struct Quality {
        quality: u32,
    }
}

// stuff in movie
make_atom! {
    #[derive(Debug)]
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

    #[derive(Debug)]
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

    #[derive(Debug)]
    struct Color {
        #[reserved]
        reserved: u16,
        red: u16,
        green: u16,
        blue: u16,
    }

    #[derive(Debug)]
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
    #[derive(Debug)]
    #[atom("clip")]
    struct Clipping {
        #[children]
        enum Child {
            ClippingRegion,
        }
    }

    #[derive(Debug)]
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
    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("matt")]
    struct TrackMatte {
        #[children]
        enum Child {
            CompressedMatte,
        }
    }

    #[derive(Debug)]
    #[version(0)]
    #[atom("kmat")]
    struct CompressedMatte {
        #[full_box]
        struct Flags {
            // unused
        },
        image_description: atom_parser::AnonymousAtom<VideoSampleDescription>,
        #[payload]
        data: Vec<u8>,
    }

    #[derive(Debug)]
    #[atom("edts")]
    struct Edit {
        #[children]
        enum Child {
            EditList,
        }
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    struct EditListEntry {
        duration: u32,
        media_time: u32,
        media_rate: u32,
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("tmcd")]
    struct TimeCode {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[derive(Debug)]
    #[atom("chap")]
    struct ChapterList {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[derive(Debug)]
    #[atom("sync")]
    struct Synchonization {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[derive(Debug)]
    #[atom("scpt")]
    struct Transcript {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[derive(Debug)]
    #[atom("ssrc")]
    struct NonprimarySource {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[derive(Debug)]
    #[atom("hint")]
    struct Hint {
        #[trailing_array]
        related_track_ids: u32,
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("imap")]
    struct TrackInputMap {
        #[children]
        enum Child {
            TrackInput,
        }
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom(b"\0\0ty")]
    struct InputType {
        ty: u32,
    }

    #[derive(Debug)]
    #[atom("obid")]
    struct ObjectId {
        object_id: u32,
    }

    #[derive(Debug)]
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

const MEDIA_COMPONENT_TYPE: FourCC = FourCC::new(*b"mhlr");
const DATA_COMPONENT_TYPE: FourCC = FourCC::new(*b"dhlr");

//media
make_atom! {
    #[derive(Debug)]
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

    #[derive(Debug, PartialEq)]
    #[enum_repr(FourCC)]
    enum MediaComponentSubtype {
        Video = FourCC::new(*b"vide"),
        Audio = FourCC::new(*b"soun"),
        Timecode = FourCC::new(*b"tmcd"),
        Text = FourCC::new(*b"text"),
        Music = FourCC::new(*b"musi"),
        Mpeg = FourCC::new(*b"MPEG"),
        Sprite =  FourCC::new(*b"sprt"),
        Tween = FourCC::new(*b"twen"),
        Q3d =  FourCC::new(*b"qd3d"),
        Streaming = FourCC::new(*b"strm"),
        Hint = FourCC::new(*b"hint"),
        Qtvr = FourCC::new(*b"qtvr"),
        Panoramic = FourCC::new(*b"pano"),
        Object = FourCC::new(*b"obje"),
    }

    #[derive(Debug)]
    #[enum_repr(FourCC)]
    enum DataComponentSubtype {
        Alias = FourCC::new(*b"alis"),
    }

    #[derive(Debug)]
    #[version(0)]
    #[atom("hdlr")]
    struct HandlerReference {
        #[full_box]
        struct Flags {
            // empty
        },
        #[condition]
        component_type_identifier: FourCC,
        #[conditional(ComponentType)]
        component_type: {
            match component_type_identifier {
                MEDIA_COMPONENT_TYPE => {
                    Media {
                        subtype: MediaComponentSubtype,
                    }
                }
                DATA_COMPONENT_TYPE => {
                    Data {
                        subtype: DataComponentSubtype,
                    }
                }
                _ => {
                    Unknown {
                        #[reuse_condition]
                        component_type_identifier: FourCC,
                        subtype: FourCC,
                    }
                }
            }
        },
        #[reserved]
        component_manufacturer: u32,
        #[reserved]
        component_flags: u32,
        #[reserved]
        component_flags_mask: u32,
        #[pascal_string(u8)]
        component_name: String,
    }

    #[derive(Debug)]
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

impl<O: atom_parser::Offset> mvhd::v0::MovieHeaderV0<O> {
    pub fn duration_from_step(&self, step: u32) -> core::time::Duration {
        let time = u32::min(self.duration, step) as u64;
        core::time::Duration::from_nanos(time * 1000000000 / (self.time_scale as u64))
    }

    pub fn duration(&self) -> core::time::Duration {
        let dur = self.duration as u64;
        core::time::Duration::from_nanos(dur * 1000000000 / (self.time_scale as u64))
    }
}

// media information
make_atom! {
    #[derive(Debug)]
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

    #[derive(Debug)]
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

    #[derive(Debug)]
    #[atom("gmhd")]
    struct BaseMediaInformationHeader {
        // empty...
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
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

impl<D: atom_parser::Parsed, O> core::fmt::Debug for SampleDescriptionEntry<D, O>
where
    <D as atom_parser::Parsed>::Output<O>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SampleDescriptionEntry")
            .field("data_reference_index", &self.data_reference_index)
            .field("description", &self.description)
            .finish()
    }
}

// sample table
make_atom! {
    #[derive(Debug)]
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

    struct UnknownSampleDesciption {
        #[payload]
        payload: Vec<u8>
    }

    #[derive(Debug)]
    #[version(0)]
    #[atom("stsd")]
    struct SampleDescription {
        #[full_box]
        struct Flags {
            // empty
        },

        #[dynamic_array(size_type = u32, zero_relative = false)]
        descriptions: atom_parser::AnonymousAtom<SampleDescriptionEntry<UnknownSampleDesciption>>
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
    struct TimeToSampleTableEntry {
        sample_count: u32,
        sample_duration: u32,
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
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

    #[derive(Debug)]
    struct SampleToChunkTableEntry {
        first_chunk: u32,
        samples_per_chunk: u32,
        sample_description_id: u32,
    }

    #[derive(Debug)]
    #[version(0)]
    #[atom("stsz")]
    struct SampleSize {
        #[full_box]
        struct Flags {
            // empty
        },
        #[condition]
        uniform_sample_size: u32,
        #[conditional(SampleSize)]
        sample_size: {
            if uniform_sample_size > 0 {
                Uniform {
                    #[reuse_condition]
                    uniform_sample_size: u32,
                }
            } else {
                Table {
                    #[dynamic_array(size_type = u32, zero_relative = false)]
                    sample_size_table: u32,
                }
            }
        }
    }
}

macro_rules! sample_description_impl {
    ($(($name:ident, $async_name:ident) => $desc:ident,)*) => {
        $(
            pub fn $name <R: atom_parser::Reader<O>>(self, reader: R, opts: &atom_parser::ParseOptions) -> Result<$desc<O>, atom_parser::ParseError> {
                self.as_description(reader, opts)
            }

            pub async fn $async_name <R: atom_parser::AsyncReader<O>>(self, reader: R, opts: &atom_parser::ParseOptions) -> Result<$desc<O>, atom_parser::ParseError> where O: Unpin {
                self.as_description_async(reader, opts).await
            }
        )*
    }
}

impl<O: atom_parser::Offset> SampleDescriptionEntry<UnknownSampleDesciption, O> {
    pub fn as_video_entry<R: atom_parser::Reader<O>>(
        self,
        fourcc: FourCC,
        reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<VideoSampleEntry<O>, atom_parser::ParseError> {
        VideoSampleEntry::parse_from_unknown(self, fourcc, reader, opts)
    }

    pub async fn as_video_entry_async<R: atom_parser::AsyncReader<O>>(
        self,
        fourcc: FourCC,
        reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<VideoSampleEntry<O>, atom_parser::ParseError>
    where
        O: Unpin,
    {
        VideoSampleEntry::parse_from_unknown_async(self, fourcc, reader, opts).await
    }

    pub fn as_audio_entry<R: atom_parser::Reader<O>>(
        self,
        fourcc: FourCC,
        reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<SoundSampleEntry<O>, atom_parser::ParseError> {
        SoundSampleEntry::parse_from_unknown(self, fourcc, reader, opts)
    }

    pub async fn as_audio_entry_async<R: atom_parser::AsyncReader<O>>(
        self,
        fourcc: FourCC,
        reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<SoundSampleEntry<O>, atom_parser::ParseError>
    where
        O: Unpin,
    {
        SoundSampleEntry::parse_from_unknown_async(self, fourcc, reader, opts).await
    }

    fn as_description<R: atom_parser::Reader<O>, T: atom_parser::Parse<O>>(
        self,
        reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<SampleDescriptionEntry<T, O>, atom_parser::ParseError> {
        let mut reader =
            atom_parser::BacktrackReader::from_extent(reader, self.description.payload.extent());
        let description = <T as atom_parser::parse::Parse<O>>::parse(&mut reader, &opts)?;

        Ok(SampleDescriptionEntry {
            data_reference_index: self.data_reference_index,
            description,
            _offset: core::marker::PhantomData,
        })
    }

    async fn as_description_async<
        R: atom_parser::AsyncReader<O> + Unpin,
        T: atom_parser::AsyncParse<O>,
    >(
        self,
        reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<SampleDescriptionEntry<T, O>, atom_parser::ParseError>
    where
        O: Unpin,
        <T as atom_parser::Parsed>::Output<O>: Unpin,
    {
        let mut reader =
            atom_parser::BacktrackReader::from_extent(reader, self.description.payload.extent());
        let description =
            <T as atom_parser::parse::AsyncParse<O>>::parse_async(&mut reader, &opts).await?;

        Ok(SampleDescriptionEntry {
            data_reference_index: self.data_reference_index,
            description,
            _offset: core::marker::PhantomData,
        })
    }

    sample_description_impl! {
        (as_video_description, as_video_description_async) => VideoSampleDescription,
        (as_audio_description, as_audio_description_async) => SoundSampleDescription,
        (as_timecode_description, as_timecode_description_async) => TimecodeSampleDescription,
        (as_text_description, as_text_description_async) => TextSampleDescription,
        (as_music_description, as_music_description_async) => MusicSampleDescription,
        (as_mpeg_description, as_mpeg_description_async) => MpegSampleDescription,
        (as_sprite_description, as_sprite_description_async) => SpriteSampleDescription,
        (as_tween_description, as_tween_description_async) => TweenSampleDescription,
        (as_3d_description, as_3d_description_async) => Q3DSampleDescription,
        (as_streaming_description, as_streaming_description_async) => StreamingSampleDescription,
        (as_hint_description, as_hint_description_async) => HintSampleDescription,
        (as_vr_description, as_vr_description_async) => QtVRSampleDescription,
        (as_panoramic_description, as_panoramic_description_async) => PanoramicSampleDescription,
        (as_object_description, as_object_description_async) => ObjectSampleDescription,
    }
}

impl<O: atom_parser::Offset> stsz::v0::SampleSizeV0<O> {
    pub fn table_len(&self) -> Option<u32> {
        match &self.sample_size {
            stsz::v0::SampleSize::Table(table) => Some(*table.sample_size_table.len()),
            _ => None,
        }
    }

    fn sample_size_from_offset<R: atom_parser::Reader<O>>(
        &self,
        start_offset: u32,
        frame_idx: u32,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<atom_parser::Extent<u32, u32>, atom_parser::ParseError> {
        match &self.sample_size {
            stsz::v0::SampleSize::Table(table) => {
                let mut table_iter = table.sample_size_table(&mut reader, opts);
                // FIXME: replace with advance_by once its not feature gated
                for _ in 0..start_offset {
                    let _ = table_iter.next().unwrap()?;
                }

                let mut sample_size = table_iter.next().unwrap()?;
                let mut sum = 0;
                for _ in start_offset..frame_idx {
                    sum += sample_size;
                    sample_size = table_iter.next().unwrap()?;
                }

                Ok(atom_parser::Extent {
                    offset: sum,
                    len: sample_size,
                })
            }
            stsz::v0::SampleSize::Uniform(stsz::v0::Uniform {
                uniform_sample_size,
                ..
            }) => {
                let offset = (frame_idx - start_offset) * uniform_sample_size;
                let len = *uniform_sample_size;
                Ok(atom_parser::Extent { offset, len })
            }
        }
    }

    async fn sample_size_from_offset_async<R: atom_parser::AsyncReader<O>>(
        &self,
        start_offset: u32,
        frame_idx: u32,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<atom_parser::Extent<u32, u32>, atom_parser::ParseError>
    where
        O: Unpin,
    {
        match &self.sample_size {
            stsz::v0::SampleSize::Table(table) => {
                let mut table_iter = table.sample_size_table_async(&mut reader, opts);
                use tokio_stream::StreamExt;
                // FIXME: replace with advance_by once its not feature gated
                for _ in 0..start_offset {
                    let _ = table_iter.next().await.unwrap()?;
                }

                let mut sample_size = table_iter.next().await.unwrap()?;
                let mut sum = 0;
                for _ in start_offset..frame_idx {
                    sum += sample_size;
                    sample_size = table_iter.next().await.unwrap()?;
                }

                Ok(atom_parser::Extent {
                    offset: sum,
                    len: sample_size,
                })
            }
            stsz::v0::SampleSize::Uniform(stsz::v0::Uniform {
                uniform_sample_size,
                ..
            }) => {
                let offset = (frame_idx - start_offset) * uniform_sample_size;
                let len = *uniform_sample_size;
                Ok(atom_parser::Extent { offset, len })
            }
        }
    }
}

// data information
make_atom! {
    #[derive(Debug)]
    #[atom("dinf")]
    struct DataInformation {
        #[children]
        enum Child {
            DataReference,
        }
    }

    #[derive(Debug)]
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

    #[derive(Debug)]
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

    #[derive(Debug)]
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

    #[derive(Debug)]
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

    #[derive(Debug)]
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

impl<O: atom_parser::Offset> stsc::v0::SampleToChunkV0<O> {
    pub fn get_frame_offset<R: atom_parser::Reader<O>>(
        &self,
        frame_idx: u32,
        chunk_offsets: &stco::v0::ChunkOffsetV0<O>,
        sample_sizes: &stsz::v0::SampleSizeV0<O>,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<atom_parser::Extent<u32, u32>, atom_parser::ParseError> {
        let frame_idx = match sample_sizes.table_len() {
            Some(len) if frame_idx > len - 1 => len - 1,
            _ => frame_idx,
        };

        let mut sample_to_chunk_iter = self.sample_to_chunk_table(&mut reader, opts);
        let mut sample_offset = 0;

        let mut primary_sample_to_chunk = sample_to_chunk_iter.next().unwrap().unwrap();

        while let Some(secondary) = sample_to_chunk_iter.next() {
            let secondary_sample_to_chunk = secondary.unwrap();
            let sample_end = sample_offset
                + (secondary_sample_to_chunk.first_chunk - primary_sample_to_chunk.first_chunk)
                    * primary_sample_to_chunk.samples_per_chunk;
            if (sample_offset..sample_end).contains(&frame_idx) {
                let reader = &mut sample_to_chunk_iter.reader;
                return Self::get_sample_offset_from_chunks(
                    frame_idx,
                    sample_offset,
                    secondary_sample_to_chunk.first_chunk - primary_sample_to_chunk.first_chunk,
                    &primary_sample_to_chunk,
                    chunk_offsets,
                    sample_sizes,
                    reader,
                    opts,
                );
            }
            primary_sample_to_chunk = secondary_sample_to_chunk;
            sample_offset = sample_end;
        }
        drop(sample_to_chunk_iter);

        let chunk_count =
            chunk_offsets.chunk_offset_table.len() - primary_sample_to_chunk.first_chunk + 1;
        let sample_end = sample_offset + chunk_count * primary_sample_to_chunk.samples_per_chunk;

        if (sample_offset..sample_end).contains(&frame_idx) {
            return Self::get_sample_offset_from_chunks(
                frame_idx,
                sample_offset,
                chunk_count,
                &primary_sample_to_chunk,
                chunk_offsets,
                sample_sizes,
                &mut reader,
                opts,
            );
        }
        unreachable!()
    }

    pub fn get_sample_offset_from_chunks<R: atom_parser::Reader<O>>(
        frame_idx: u32,
        sample_offset: u32,
        chunk_count: u32,
        entry: &SampleToChunkTableEntry<O>,
        chunk_offsets: &stco::v0::ChunkOffsetV0<O>,
        sample_sizes: &stsz::v0::SampleSizeV0<O>,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<atom_parser::Extent<u32, u32>, atom_parser::ParseError> {
        let mut sample_start = sample_offset;

        for i in 0..chunk_count {
            let sample_end = sample_start + entry.samples_per_chunk;
            if (sample_start..sample_end).contains(&frame_idx) {
                let chunk_base = chunk_offsets
                    .chunk_offset_table(&mut *reader, opts)
                    .nth((entry.first_chunk + i - 1) as usize)
                    .unwrap()?;
                let sample_extent = sample_sizes.sample_size_from_offset(
                    sample_start,
                    frame_idx,
                    &mut *reader,
                    opts,
                )?;

                return Ok(atom_parser::Extent {
                    offset: chunk_base + sample_extent.offset,
                    len: sample_extent.len,
                });
            }
            sample_start = sample_end;
        }
        unreachable!()
    }

    pub async fn get_frame_offset_async<R: atom_parser::AsyncReader<O>>(
        &self,
        frame_idx: u32,
        chunk_offsets: &stco::v0::ChunkOffsetV0<O>,
        sample_sizes: &stsz::v0::SampleSizeV0<O>,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<atom_parser::Extent<u32, u32>, atom_parser::ParseError>
    where
        O: Unpin,
    {
        let frame_idx = match sample_sizes.table_len() {
            Some(len) if frame_idx > len - 1 => len - 1,
            _ => frame_idx,
        };

        use tokio_stream::StreamExt;
        let mut sample_to_chunk_iter = self.sample_to_chunk_table_async(&mut reader, opts);
        let mut sample_offset = 0;

        let mut primary_sample_to_chunk = sample_to_chunk_iter.next().await.unwrap().unwrap();

        while let Some(secondary) = sample_to_chunk_iter.next().await {
            let secondary_sample_to_chunk = secondary.unwrap();
            let sample_end = sample_offset
                + (secondary_sample_to_chunk.first_chunk - primary_sample_to_chunk.first_chunk)
                    * primary_sample_to_chunk.samples_per_chunk;
            if (sample_offset..sample_end).contains(&frame_idx) {
                let reader = sample_to_chunk_iter.reader();
                return Self::get_sample_offset_from_chunks_async(
                    frame_idx,
                    sample_offset,
                    secondary_sample_to_chunk.first_chunk - primary_sample_to_chunk.first_chunk,
                    &primary_sample_to_chunk,
                    chunk_offsets,
                    sample_sizes,
                    reader,
                    opts,
                )
                .await;
            }
            primary_sample_to_chunk = secondary_sample_to_chunk;
            sample_offset = sample_end;
        }
        drop(sample_to_chunk_iter);

        let chunk_count =
            chunk_offsets.chunk_offset_table.len() - primary_sample_to_chunk.first_chunk + 1;
        let sample_end = sample_offset + chunk_count * primary_sample_to_chunk.samples_per_chunk;

        if (sample_offset..sample_end).contains(&frame_idx) {
            return Self::get_sample_offset_from_chunks_async(
                frame_idx,
                sample_offset,
                chunk_count,
                &primary_sample_to_chunk,
                chunk_offsets,
                sample_sizes,
                &mut reader,
                opts,
            )
            .await;
        }
        unreachable!()
    }

    pub async fn get_sample_offset_from_chunks_async<R: atom_parser::AsyncReader<O>>(
        frame_idx: u32,
        sample_offset: u32,
        chunk_count: u32,
        entry: &SampleToChunkTableEntry<O>,
        chunk_offsets: &stco::v0::ChunkOffsetV0<O>,
        sample_sizes: &stsz::v0::SampleSizeV0<O>,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<atom_parser::Extent<u32, u32>, atom_parser::ParseError>
    where
        O: Unpin,
    {
        let mut sample_start = sample_offset;

        for i in 0..chunk_count {
            let sample_end = sample_start + entry.samples_per_chunk;
            if (sample_start..sample_end).contains(&frame_idx) {
                let chunk_base = {
                    use tokio_stream::StreamExt;
                    let mut iter = chunk_offsets.chunk_offset_table_async(&mut *reader, opts);
                    let mut base = iter.next().await;
                    for _ in 0..(entry.first_chunk + i - 1) as usize {
                        base = iter.next().await;
                    }
                    base.unwrap()?
                };

                let sample_extent = sample_sizes
                    .sample_size_from_offset_async(sample_start, frame_idx, &mut *reader, opts)
                    .await?;

                return Ok(atom_parser::Extent {
                    offset: chunk_base + sample_extent.offset,
                    len: sample_extent.len,
                });
            }
            sample_start = sample_end;
        }
        unreachable!()
    }
}

// video media
pub mod sample_description {
    use super::*;
    make_atom! {
        #[derive(Debug)]
        // this isnt an atom, but it has children so it is treated like one
        #[atom(b"fake")]
        struct Video {
            version: u16,
            #[reserved]
            revision_level: u16,
            vendor: FourCC,
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
            #[pascal_string(u8)]
            compressor_name: String,
            pixel_depth: u16,
            color_table_id: u16,
            // named extensions of the video sample description
            #[children]
            enum Child {
                Gamma,
                FieldHandling,
                MjpegDefaultQuantizationTable,
                MjpegDefaultHuffmanTable,
            }
        }

        #[derive(Debug)]
        #[atom("gama")]
        struct Gamma {
            gamma_level: u32,
        }

        #[derive(Debug)]
        #[enum_repr(u8)]
        enum FieldOrdering {
            UnknownFieldOrdering = 0,
            // note whichever is first is also stored first in the file
            TopmostFirst = 1,
            BottommostFirst = 6,
        }

        #[derive(Debug)]
        #[atom("fiel")]
        struct FieldHandling {
            #[condition]
            field_count: u8,
            #[conditional]
            ordering: {
                if field_count == 2 {
                    Ordering {
                        ordering: FieldOrdering,
                    }
                }
            }
        }

        #[derive(Debug)]
        #[atom("mjqt")]
        struct MjpegDefaultQuantizationTable {
            #[payload]
            quantization_table: Vec<u8>
        }

        #[derive(Debug)]
        #[atom("mjht")]
        struct MjpegDefaultHuffmanTable {
            #[payload]
            huffman_table: Vec<u8>,
        }

        #[derive(Debug)]
        struct Sound {
            #[version]
            version: u16,
            revision_level: u16,
            vendor: u32,
            number_of_channels: u16,
            sample_size: u16,
            compression_id: u16,
            packet_size: u16,
            sample_rate: u32,
            #[versions]
            enum Version {
                #[version(0)]
                V1 {
                    // empty
                },
                #[version(1)]
                V2 {
                    samples_per_packet: u32,
                    bytes_per_packet: u32,
                    bytes_per_frame: u32,
                    bytes_per_sample: u32,
                },
            }
        }

        #[derive(Debug)]
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

        #[derive(Debug)]
        #[atom("fak2")]
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
            // extensions
            #[children]
            enum Child {
                TextStyleInformation,
                FontNameTable,
                HighlightInformation,
                HighlightColor,
                DropShadowOffset,
                DropShadowTransparency,
                ImageFontData,
                ImageFontHighlighting,
            }
        }

        #[derive(Debug)]
        #[atom("styl")]
        struct TextStyleInformation {
            #[dynamic_array(size_type = u16, zero_relative = false)]
            entries: TextStyleTableEntry,
        }

        // from ScrapSTElement in TextEdit docs
        struct TextStyleTableEntry {
            start_char: u32,
            height: u16,
            ascent: u16,
            font_id: u16,
            #[flags(u16)]
            flags: struct Flags {
                const BOLD = 1 << 0;
                const ITALIC = 1 << 1;
                const UNDERLINE = 1 << 2;
                const OUTLINE = 1 << 3;
                const SHADOW = 1 << 4;
                const CONDENSE = 1 << 5;
                const EXTEND = 1 << 6;
            },
            font_size: u16,
            fg_color: [u16; 3],
        }

        #[derive(Debug)]
        struct FontTableEntry {
            font_number: u16,
            #[pascal_string(u8)]
            font_name: String,
        }

        #[derive(Debug)]
        #[atom("ftab")]
        struct FontNameTable {
            #[trailing_array]
            entries: FontTableEntry,
        }

        #[derive(Debug)]
        #[atom("hlit")]
        struct HighlightInformation {
            highlight_starting_offset: u32,
            highlight_ending_offset: u32,
        }

        #[derive(Debug)]
        #[atom("hclr")]
        struct HighlightColor {
            color: [u16; 3]
        }

        #[derive(Debug)]
        #[atom("drpo")]
        struct DropShadowOffset {
            horizontal_offset: u16,
            vertical_offset: u16,
        }

        #[derive(Debug)]
        #[atom("drpt")]
        struct DropShadowTransparency {
            transparency: u16,
        }

        #[derive(Debug)]
        #[atom("imag")]
        struct ImageFontData {
            #[children]
            enum Child {
                ImageDescription,
                ImageData,
            }
        }

        #[derive(Debug)]
        #[atom("metr")]
        struct ImageFontHighlighting {
            // empty
        }

        #[derive(Debug)]
        struct Music {
            #[flags(u32)]
            flags: struct Flags {
                // empty
            }
        }

        #[derive(Debug)]
        struct Mpeg {
            // empty
        }

        #[derive(Debug)]
        struct Sprite {
            // empty
        }

        #[derive(Debug)]
        struct Tween {
            // empty
        }

        #[derive(Debug)]
        struct Q3D {
            // empty
        }

        #[derive(Debug)]
        struct Streaming {
            version: u32,
            #[reserved]
            reserved: [u8; 4],
            flags: u32,
        }

        #[derive(Debug)]
        #[atom("fak3")]
        struct Hint {
            version: u16,
            last_compatible_version: u16,
            max_packet_size: u32,

            #[children]
            enum Child {
                RtpTimescale,
                RtpTimestampOffset,
                RtpSequenceNumberOffset,
            }
        }

        #[derive(Debug)]
        #[atom("tims")]
        struct RtpTimescale {
            timescale: u32,
        }

        #[derive(Debug)]
        #[atom("tsro")]
        struct RtpTimestampOffset {
            offset: u32,
        }

        #[derive(Debug)]
        #[atom("snro")]
        struct RtpSequenceNumberOffset {
            offset: u32,
        }

        #[derive(Debug)]
        struct QuickTimeVR {
            //
        }

        #[derive(Debug)]
        struct Panoramic {

        }

        #[derive(Debug)]
        struct Object {

        }
    }
}

pub type VideoSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Video, O>;
pub type SoundSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Sound, O>;
pub type TimecodeSampleDescription<O = usize> =
    SampleDescriptionEntry<sample_description::Timecode, O>;
pub type TextSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Text, O>;
pub type MusicSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Music, O>;
pub type MpegSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Mpeg, O>;
pub type SpriteSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Sprite, O>;
pub type TweenSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Tween, O>;
pub type Q3DSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Q3D, O>;
pub type StreamingSampleDescription<O = usize> =
    SampleDescriptionEntry<sample_description::Streaming, O>;
pub type HintSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Hint, O>;

// note: all of these have to do with qtvr
pub type QtVRSampleDescription<O = usize> =
    SampleDescriptionEntry<sample_description::QuickTimeVR, O>;
pub type PanoramicSampleDescription<O = usize> =
    SampleDescriptionEntry<sample_description::Panoramic, O>;
pub type ObjectSampleDescription<O = usize> = SampleDescriptionEntry<sample_description::Object, O>;

const SDP_CONTENT_TYPE: FourCC = FourCC::new(*b"sdp ");

// hints
make_atom! {
    #[derive(Debug)]
    #[atom("hnti")]
    struct MovieHintInfo {
        #[children]
        enum Child {
            RtpHint,
        }
    }

    #[derive(Debug)]
    #[atom("hinf")]
    struct HintTrackStatistics {
        #[children]
        enum Child {
            TrackPayloadSizeHint64,
            TrackPayloadSizeHint32,
            NetworkPacketHint64,
            NetworkPacketHint32,
            ByteCountHint64,
            ByteCountHint32,
            DatarateHint,
            MediaTrackBytesHint,
            ImmediateBytesHint,
            RepeatedBytesHint,
            MinTransmissionTimeHint,
            MaxTransmissionTimeHint,
            LargestPacketHint,
            LargestPacketDurationHint,
            PayloadTypeHint,
        }
    }

    #[derive(Debug)]
    #[atom("rtp ")]
    struct RtpHint {
        #[condition]
        data_type: FourCC,
        #[conditional]
        data: {
            if data_type == SDP_CONTENT_TYPE {
                Sdp {
                    #[payload]
                    session: String,
                }
            }
        }
    }

    #[derive(Debug)]
    #[atom("trpY")]
    struct TrackPayloadSizeHint64 {
        byte_len: u64,
    }

    #[derive(Debug)]
    #[atom("totl")]
    struct TrackPayloadSizeHint32 {
        byte_len: u32,
    }

    #[derive(Debug)]
    #[atom("nump")]
    struct NetworkPacketHint64 {
        network_packet_count: u64,
    }

    #[derive(Debug)]
    #[atom("npck")]
    struct NetworkPacketHint32 {
        network_packet_count: u32,
    }

    #[derive(Debug)]
    #[atom("tpyl")]
    struct ByteCountHint64 {
        total_bytes_minus_rtp_headers: u64,
    }

    #[derive(Debug)]
    #[atom("tpaY")]
    struct ByteCountHint32 {
        total_bytes_minus_rtp_headers: u32,
    }

    #[derive(Debug)]
    #[atom("maxr")]
    struct DatarateHint {
        granularity_ms: u32,
        maximum_datarate: u32,
    }

    #[derive(Debug)]
    #[atom("dmed")]
    struct MediaTrackBytesHint {
        byte_count: u64,
    }

    #[derive(Debug)]
    #[atom("dimm")]
    struct ImmediateBytesHint {
        byte_count: u64,
    }

    #[derive(Debug)]
    #[atom("drep")]
    struct RepeatedBytesHint {
        byte_count: u64,
    }

    #[derive(Debug)]
    #[atom("tmin")]
    struct MinTransmissionTimeHint {
        shortest_transmission_ms: u32,
    }

    #[derive(Debug)]
    #[atom("tmax")]
    struct MaxTransmissionTimeHint {
        longest_transmission_ms: u32,
    }

    #[derive(Debug)]
    #[atom("pmax")]
    struct LargestPacketHint {
        byte_count: u32,
    }

    #[derive(Debug)]
    #[atom("dmax")]
    struct LargestPacketDurationHint {
        largest_duration_ms: u32,
    }

    #[derive(Debug)]
    #[atom("payt")]
    struct PayloadTypeHint {
        payload_number: u32,
        #[pascal_string(u8)]
        rtpmap: String,
    }
}

// image format
make_atom! {
    #[derive(Debug)]
    #[atom("idsc")]
    struct ImageDescription {
        description: atom_parser::AnonymousAtom<VideoSampleDescription>,
    }

    #[derive(Debug)]
    #[atom("idat")]
    struct ImageData {
        #[payload]
        data: Vec<u8>,
    }
}

#[derive(Debug)]
pub enum MediaKind<O> {
    Video {
        header: VideoMediaInformationHeader<O>,
        handler_ref: HandlerReference<O>,
    },
    Audio {
        header: SoundMediaInformationHeader<O>,
        handler_ref: HandlerReference<O>,
    },
    Base {
        header: BaseMediaInformationHeader<O>,
        info: BaseMediaInformation<O>,
        timecode: Option<TimecodeMediaInformation<O>>,
        handler_ref: Option<HandlerReference<O>>,
    },
}

#[derive(Debug)]
pub struct DiscoveredMediaInformation<O> {
    pub kind: MediaKind<O>,
    pub data_information: Option<DataInformation<O>>,
    pub sample_table: Option<SampleTable<O>>,
}

impl<O: atom_parser::Offset> DiscoveredMediaInformation<O>
where
    minf::Child<O>: atom_parser::Parse<O, Output<O> = minf::Child<O>>,
    atom_parser::children::UnknownChild<O>: core::fmt::Debug,
{
    pub fn from_children<R: atom_parser::Reader<O>>(
        media_information: &MediaInformation<O>,
        mut r: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        let mut child_iter = media_information.children(&mut r, opts);

        let mut video_header = None;
        let mut audio_header = None;
        let mut base_header = None;
        let mut base_info = None;
        let mut timecode_info = None;

        let mut handler_ref = None;
        let mut data_information = None;
        let mut sample_table = None;

        while let Some(child) = child_iter.next() {
            match child? {
                minf::Child::VideoMediaInformationHeader(vid) => {
                    if video_header.is_some() {
                        panic!("duplicate video information headers")
                    }
                    video_header = Some(vid)
                }
                minf::Child::SoundMediaInformationHeader(snd) => {
                    if audio_header.is_some() {
                        panic!("duplicate audio information headers")
                    }
                    audio_header = Some(snd)
                }
                minf::Child::TimecodeMediaInformation(info) => {
                    if timecode_info.is_some() {
                        panic!("duplicate timecode informations")
                    }
                    timecode_info = Some(info)
                }
                minf::Child::BaseMediaInformationHeader(base) => {
                    if base_header.is_some() {
                        panic!("duplicate base information headers")
                    }
                    base_header = Some(base)
                }
                minf::Child::BaseMediaInformation(base) => {
                    if base_info.is_some() {
                        panic!("duplicate base informations")
                    }
                    base_info = Some(base)
                }
                minf::Child::HandlerReference(href) => {
                    if handler_ref.is_some() {
                        panic!("duplicate handler references")
                    }

                    handler_ref = Some(href)
                }
                minf::Child::DataInformation(dinfo) => {
                    if data_information.is_some() {
                        panic!("duplicate data information")
                    }
                    data_information = Some(dinfo)
                }
                minf::Child::SampleTable(stable) => {
                    if sample_table.is_some() {
                        panic!("duplicate sample tables")
                    }
                    sample_table = Some(stable)
                }
                minf::Child::Unsupported(c) => println!("unknown minf child: {c:?}"),
            }
        }
        Self::from_options(
            video_header,
            audio_header,
            handler_ref,
            base_header,
            base_info,
            timecode_info,
            data_information,
            sample_table,
        )
    }

    pub async fn from_children_async<R: atom_parser::AsyncReader<O> + Unpin>(
        media_information: &MediaInformation<O>,
        mut r: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError>
    where
        O: Unpin,
    {
        let mut child_iter = media_information.children_async(&mut r, opts);

        let mut video_header = None;
        let mut audio_header = None;
        let mut base_header = None;
        let mut base_info = None;
        let mut timecode_info = None;

        let mut handler_ref = None;
        let mut data_information = None;
        let mut sample_table = None;

        use tokio_stream::StreamExt;
        while let Some(child) = child_iter.next().await {
            match child? {
                minf::Child::VideoMediaInformationHeader(vid) => {
                    if video_header.is_some() {
                        panic!("duplicate video information headers")
                    }
                    video_header = Some(vid)
                }
                minf::Child::SoundMediaInformationHeader(snd) => {
                    if audio_header.is_some() {
                        panic!("duplicate audio information headers")
                    }
                    audio_header = Some(snd)
                }
                minf::Child::TimecodeMediaInformation(info) => {
                    if timecode_info.is_some() {
                        panic!("duplicate timecode informations")
                    }
                    timecode_info = Some(info)
                }
                minf::Child::BaseMediaInformationHeader(base) => {
                    if base_header.is_some() {
                        panic!("duplicate base information headers")
                    }
                    base_header = Some(base)
                }
                minf::Child::BaseMediaInformation(base) => {
                    if base_info.is_some() {
                        panic!("duplicate base informations")
                    }
                    base_info = Some(base)
                }
                minf::Child::HandlerReference(href) => {
                    if handler_ref.is_some() {
                        panic!("duplicate handler references")
                    }

                    handler_ref = Some(href)
                }
                minf::Child::DataInformation(dinfo) => {
                    if data_information.is_some() {
                        panic!("duplicate data information")
                    }
                    data_information = Some(dinfo)
                }
                minf::Child::SampleTable(stable) => {
                    if sample_table.is_some() {
                        panic!("duplicate sample tables")
                    }
                    sample_table = Some(stable)
                }
                minf::Child::Unsupported(c) => println!("unknown minf child: {c:?}"),
            }
        }
        Self::from_options(
            video_header,
            audio_header,
            handler_ref,
            base_header,
            base_info,
            timecode_info,
            data_information,
            sample_table,
        )
    }

    fn from_options(
        video_header: Option<VideoMediaInformationHeader<O>>,
        audio_header: Option<SoundMediaInformationHeader<O>>,
        handler_ref: Option<HandlerReference<O>>,
        base_header: Option<BaseMediaInformationHeader<O>>,
        base_info: Option<BaseMediaInformation<O>>,
        timecode_info: Option<TimecodeMediaInformation<O>>,
        data_information: Option<DataInformation<O>>,
        sample_table: Option<SampleTable<O>>,
    ) -> Result<Self, atom_parser::ParseError> {
        Ok(
            match (
                video_header,
                audio_header,
                handler_ref,
                base_header,
                base_info,
                timecode_info,
            ) {
                (Some(video), None, Some(handler_ref), None, None, None) => Self {
                    kind: MediaKind::Video {
                        header: video,
                        handler_ref,
                    },
                    data_information,
                    sample_table,
                },
                (None, Some(audio), Some(handler_ref), None, None, None) => Self {
                    kind: MediaKind::Audio {
                        header: audio,
                        handler_ref,
                    },
                    data_information,
                    sample_table,
                },
                (None, None, handler_ref, Some(base_header), Some(base_info), timecode) => Self {
                    kind: MediaKind::Base {
                        header: base_header,
                        info: base_info,
                        handler_ref,
                        timecode,
                    },
                    data_information,
                    sample_table,
                },
                _ => {
                    panic!("bad minf state");
                }
            },
        )
    }
}

make_atom! {
    #[derive(Debug)]
    #[enum_repr(FourCC)]
    enum AudioCodec {
        NotCompressed = FourCC::new(*b"NONE"),
        Uncompressed8Bit = FourCC::new(*b"raw "),
        Uncompressed16BitBe = FourCC::new(*b"twos"),
        Uncompressed16BitLe = FourCC::new(*b"sowt"),
        Mace3 = FourCC::new(*b"MAC3"),
        Mace6 = FourCC::new(*b"MAC6"),
        Ima4To1 = FourCC::new(*b"ima4"),
        Float32 = FourCC::new(*b"fl32"),
        Float64 = FourCC::new(*b"fl64"),
        Int24 = FourCC::new(*b"in24"),
        Int32 = FourCC::new(*b"in32"),
        Ulaw2To1 = FourCC::new(*b"ulaw"),
        Alaw2To1 = FourCC::new(*b"alaw"),
        MsAdpcmAcm = FourCC::from_be_bits(0x6D730002),
        DviIntelImaadcpmAcm = FourCC::from_be_bits(0x6D730011),
        DvAudio = FourCC::new(*b"dvca"),
        QDesignMusic = FourCC::new(*b"QDMC"),
        QDesignMusicV2 = FourCC::new(*b"QDM2"),
        QualcommPurevoice = FourCC::new(*b"Qclp"),
        Mpeg3 = FourCC::from_be_bits(0x6D730055),
        Mpeg3Full = FourCC::new(*b".mp3"),
    }
}

#[derive(Debug)]
pub struct SoundSampleEntry<O = usize> {
    pub codec: AudioCodec,
    pub entry: SoundSampleDescription<O>,
}

impl<O: atom_parser::Offset> SoundSampleEntry<O> {
    pub fn parse_from_unknown<R: atom_parser::Reader<O>>(
        unknown: SampleDescriptionEntry<UnknownSampleDesciption, O>,
        fourcc: FourCC,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        let mut reader = atom_parser::BacktrackReader::from_extent(
            &mut reader,
            unknown.description.payload.extent(),
        );
        let entry = unknown.as_audio_description(&mut reader, opts)?;

        let codec = AudioCodec::try_from_bits(fourcc);

        Ok(Self { entry, codec })
    }

    pub async fn parse_from_unknown_async<R: atom_parser::AsyncReader<O>>(
        unknown: SampleDescriptionEntry<UnknownSampleDesciption, O>,
        fourcc: FourCC,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError>
    where
        O: Unpin,
    {
        let mut reader = atom_parser::BacktrackReader::from_extent(
            &mut reader,
            unknown.description.payload.extent(),
        );
        let entry = unknown
            .as_audio_description_async(&mut reader, opts)
            .await?;

        let codec = AudioCodec::try_from_bits(fourcc);

        Ok(Self { entry, codec })
    }
}

make_atom! {
    #[derive(Debug)]
    #[enum_repr(FourCC)]
    enum VideoCodec {
        Cinepak = FourCC::new(*b"cvid"),
        Jpeg = FourCC::new(*b"jpeg"),
        Rgb = FourCC::new(*b"raw "),
        Yuv422 = FourCC::new(*b"Yuv2"),
        Smc = FourCC::new(*b"smc "),
        Rle = FourCC::new(*b"rle "),
        RoadPizza = FourCC::new(*b"rpza"),
        KodakPhotoCd = FourCC::new(*b"kpcd"),
        Mpeg = FourCC::new(*b"mpeg"),
        MJpegA = FourCC::new(*b"mjpa"),
        MJpegB = FourCC::new(*b"mjpb"),
        SorensonVideo = FourCC::new(*b"svqi"),
    }
}

#[derive(Debug)]
pub struct VideoSampleEntry<O = usize> {
    pub codec: VideoCodec,
    pub entry: VideoSampleDescription<O>,
}

impl<O: atom_parser::Offset> VideoSampleEntry<O> {
    pub fn parse_from_unknown<R: atom_parser::Reader<O>>(
        unknown: SampleDescriptionEntry<UnknownSampleDesciption, O>,
        fourcc: FourCC,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        let mut reader = atom_parser::BacktrackReader::from_extent(
            &mut reader,
            unknown.description.payload.extent(),
        );

        let entry = unknown.as_video_description(&mut reader, opts)?;
        let codec = VideoCodec::try_from_bits(fourcc);

        Ok(Self { entry, codec })
    }

    pub async fn parse_from_unknown_async<R: atom_parser::AsyncReader<O>>(
        unknown: SampleDescriptionEntry<UnknownSampleDesciption, O>,
        fourcc: FourCC,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError>
    where
        O: Unpin,
    {
        let mut reader = atom_parser::BacktrackReader::from_extent(
            &mut reader,
            unknown.description.payload.extent(),
        );

        let entry = unknown
            .as_video_description_async(&mut reader, opts)
            .await?;
        let codec = VideoCodec::try_from_bits(fourcc);

        Ok(Self { entry, codec })
    }
}

#[derive(Debug)]
pub struct VideoTrack<O = usize> {
    pub entries: Vec<VideoSampleEntry<O>>,
}

#[derive(Debug)]
pub struct AudioTrack<O = usize> {
    pub entries: Vec<SoundSampleEntry<O>>,
}

#[derive(Debug)]
pub enum TrackKind<O = usize> {
    Audio(AudioTrack<O>),
    Video(VideoTrack<O>),
}

#[derive(Debug)]
pub struct PlayableTrack<O = usize> {
    pub kind: TrackKind<O>,
    pub time_to_sample: TimeToSample<O>,
    pub sample_size: SampleSize<O>,
    pub sample_to_chunk: SampleToChunk<O>,
    pub chunk_offset: ChunkOffset<O>,
}

#[cfg(test)]
const TEST_FILE: &str = "../raw_codec_test.mov";

#[cfg(test)]
fn create_reader() -> atom_parser::InMemoryReader<'static> {
    #[cfg(not(feature = "std"))]
    {
        let bytes = std::fs::read(TEST_FILE).unwrap();
        atom_parser::InMemoryReader::from_buf(bytes)
    }
    #[cfg(feature = "std")]
    {
        atom_parser::InMemoryReader::from_path(TEST_FILE).unwrap()
    }
}

#[test]
fn ftyp() {
    let mut r = create_reader();
    let opts = Default::default();

    use atom_parser::Parse;

    let root = Root::parse(&mut r, &opts).unwrap();
    let mut root_iter = root.children(&mut r, &opts);

    let mut tracks = vec![];
    let mut movie_data = None;
    let mut movie_header = None;

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
                            movie_header = Some(hd);
                        }
                        moov::Child::Clipping(clip) => {
                            println!("clip: {clip:?}");
                        }
                        moov::Child::Track(track) => {
                            println!("track: {track:?}");

                            let mut sample_description = None;
                            let mut time_to_sample = None;
                            let mut sample_sizes = None;
                            let mut sample_to_chunk = None;
                            let mut chunk_offset = None;
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
                                                        println!("unknown elst: {v:?}")
                                                    }
                                                },
                                                edts::Child::Unsupported(..) => (),
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
                                        let mut media_info = None;
                                        let mut handler_ref = None;
                                        let mut sample_desc_table = None;

                                        let mut child_iter = m.children(r, &opts);
                                        while let Some(child) = child_iter.next() {
                                            let r = &mut child_iter.reader;
                                            match child.unwrap() {
                                                mdia::Child::MediaHeader(h) => {
                                                    println!("media heaader: {h:?}")
                                                }
                                                mdia::Child::HandlerReference(r) => {
                                                    if handler_ref.as_ref().is_some() {
                                                        panic!("duplicate handler refs");
                                                    }
                                                    match r.version {
                                                        hdlr::HandlerReferenceVersions::V0(v0) => {
                                                            match v0.component_type {
                                                                hdlr::v0::ComponentType::Media(
                                                                    m,
                                                                ) => handler_ref = Some(m.subtype),
                                                                o => panic!(
                                                                    "unknown media type: {o:?}"
                                                                ),
                                                            }
                                                        }
                                                        v => panic!("unknown hdlr version: {v:?}"),
                                                    }
                                                }
                                                mdia::Child::MediaInformation(info) => {
                                                    println!("info: {info:?}");

                                                    let DiscoveredMediaInformation {
                                                        kind,
                                                        data_information,
                                                        sample_table,
                                                    } = DiscoveredMediaInformation::from_children(
                                                        &info, &mut *r, &opts,
                                                    )
                                                    .unwrap();

                                                    if media_info.is_some() {
                                                        panic!("duplicate media infos");
                                                    }
                                                    media_info = Some(kind);

                                                    if let Some(dinfo) = data_information {
                                                        println!("dinfo: {dinfo:?}");
                                                        let mut child_iter =
                                                            dinfo.children(r, &opts);
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
                                                                                    dref::v0::Child::Unsupported(..) => (),
                                                                                }
                                                                            }
                                                                        }
                                                                        dref::DataReferenceVersions::Unknown(v) => println!("unknown dref: {v:?}"),
                                                                    }
                                                                }
                                                                dinf::Child::Unsupported(..) => (),
                                                            }
                                                        }
                                                    }

                                                    if let Some(stable) = sample_table {
                                                        println!("stable: {stable:?}");

                                                        let mut child_iter =
                                                            stable.children(r, &opts);
                                                        while let Some(child) = child_iter.next() {
                                                            let r = &mut child_iter.reader;
                                                            match child.unwrap() {
                                                                stbl::Child::SampleDescription(desc) => {
                                                                    if sample_desc_table.is_some() {
                                                                        panic!("more than 1 stsd atom found in the same minf atom");
                                                                    }
                                                                    sample_desc_table = Some(desc);
                                                                }
                                                                stbl::Child::TimeToSample(tts) => {
                                                                    match &tts.version {
                                                                        stts::TimeToSampleVersions::V0(tts) => {
                                                                            let time_to_sample = tts.time_to_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                            println!("stts entries: {time_to_sample:?}");
                                                                        }
                                                                        stts::TimeToSampleVersions::Unknown(v) => println!("unknown stts: {v:?}"),
                                                                    }
                                                                    time_to_sample = Some(tts);
                                                                }
                                                                stbl::Child::SyncSample(sync) => {
                                                                    match sync.version {
                                                                        stss::SyncSampleVersions::V0(sync) => {
                                                                            println!("sync sample: {sync:?}");
                                                                            let sync_samples = sync.sync_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                            println!("sync sample entries: {sync_samples:?}");
                                                                        }
                                                                        stss::SyncSampleVersions::Unknown(v) => println!("unknown stts: {v:?}"),
                                                                    }
                                                                }
                                                                stbl::Child::SampleToChunk(stc) => {
                                                                    match &stc.version {
                                                                        stsc::SampleToChunkVersions::V0(stc) => {
                                                                            println!("stsc: {stc:?}");
                                                                            let sample_to_chunk = stc.sample_to_chunk_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                            println!("stsc entries: {sample_to_chunk:?}");
                                                                        }
                                                                        stsc::SampleToChunkVersions::Unknown(v) => println!("unknown stsc: {v:?}"),
                                                                    }
                                                                    sample_to_chunk = Some(stc);
                                                                }
                                                                stbl::Child::SampleSize(ss) => {
                                                                    match &ss.version {
                                                                        stsz::SampleSizeVersions::V0(ss) => {
                                                                            println!("stsz: {ss:?}");
                                                                            match &ss.sample_size {
                                                                                stsz::v0::SampleSize::Uniform(u) => println!("uniform: {u:?}"),
                                                                                stsz::v0::SampleSize::Table(st) => {
                                                                                    let sample_sizes = st.sample_size_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                    println!("stsz entries: {sample_sizes:?}");
                                                                                }
                                                                            }
                                                                        }
                                                                        stsz::SampleSizeVersions::Unknown(v) => println!("unknown stsz: {v:?}"),
                                                                    }
                                                                    sample_sizes = Some(ss);
                                                                }
                                                                stbl::Child::ChunkOffset(co) => {
                                                                    match &co.version {
                                                                        stco::ChunkOffsetVersions::V0(co) => {
                                                                            println!("co32: {co:?}");
                                                                            let offsets = co.chunk_offset_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                            println!("co32 entries: {offsets:?}")
                                                                        }
                                                                        stco::ChunkOffsetVersions::Unknown(v) => println!("unknown stco: {v:?}"),
                                                                    }
                                                                    chunk_offset = Some(co);
                                                                }
                                                                stbl::Child::Unsupported(u) => println!("unsupported stbl entry: {u:?}"),
                                                            }
                                                        }
                                                    }
                                                }
                                                mdia::Child::Userdata(u) => {
                                                    for child in u.children(r, &opts) {
                                                        println!("mdia udata: {child:?}");
                                                    }
                                                }
                                                mdia::Child::Unsupported(_) => (),
                                            }
                                        }

                                        drop(child_iter);

                                        if let (Some(media_info), Some(sample_desc_table)) =
                                            (media_info, sample_desc_table)
                                        {
                                            let stsd = match sample_desc_table.version {
                                                stsd::SampleDescriptionVersions::V0(stsd) => stsd,
                                                v => panic!("unknown stsd version: {v:?}"),
                                            };
                                            let mut desc_iter = stsd.descriptions(r, &opts);
                                            match (media_info, handler_ref) {
                                                (
                                                    MediaKind::Video {
                                                        header,
                                                        handler_ref,
                                                    },
                                                    Some(MediaComponentSubtype::Video) | None,
                                                ) => {
                                                    println!("video header: {header:?}");
                                                    println!("video handler ref: {handler_ref:?}");
                                                    let mut entries = vec![];
                                                    while let Some(desc) = desc_iter.next() {
                                                        let desc = desc.unwrap();
                                                        let entry = desc
                                                            .content
                                                            .as_video_entry(
                                                                desc.fourcc,
                                                                &mut desc_iter.reader,
                                                                &opts,
                                                            )
                                                            .unwrap();
                                                        println!("video: {entry:?}");
                                                        let mut extension_iter = entry
                                                            .entry
                                                            .description
                                                            .children(&mut desc_iter.reader, &opts);

                                                        while let Some(extension) =
                                                            extension_iter.next()
                                                        {
                                                            let extension = extension.unwrap();
                                                            println!(
                                                                "video extension: {extension:?}"
                                                            );
                                                        }
                                                        entries.push(entry);
                                                    }
                                                    sample_description =
                                                        Some(TrackKind::Video(VideoTrack {
                                                            entries,
                                                        }));
                                                }
                                                (
                                                    MediaKind::Audio {
                                                        header,
                                                        handler_ref,
                                                    },
                                                    Some(MediaComponentSubtype::Audio) | None,
                                                ) => {
                                                    println!("audio header: {header:?}");
                                                    println!("audio handler ref: {handler_ref:?}");
                                                    let mut entries = vec![];
                                                    while let Some(desc) = desc_iter.next() {
                                                        let desc = desc.unwrap();
                                                        let entry = desc
                                                            .content
                                                            .as_audio_entry(
                                                                desc.fourcc,
                                                                &mut desc_iter.reader,
                                                                &opts,
                                                            )
                                                            .unwrap();
                                                        entries.push(entry);
                                                    }
                                                    sample_description =
                                                        Some(TrackKind::Audio(AudioTrack {
                                                            entries,
                                                        }));
                                                }
                                                (
                                                    MediaKind::Base {
                                                        header,
                                                        info,
                                                        handler_ref,
                                                        timecode: Some(timecode),
                                                    },
                                                    Some(MediaComponentSubtype::Timecode),
                                                ) => {
                                                    println!("timecode base header: {header:?}");
                                                    println!("timecode base info: {info:?}");
                                                    println!(
                                                        "timecode handler ref: {handler_ref:?}"
                                                    );
                                                    println!("timecode info: {timecode:?}");
                                                    while let Some(desc) = desc_iter.next() {
                                                        let entry = desc
                                                            .unwrap()
                                                            .content
                                                            .as_timecode_description(
                                                                &mut desc_iter.reader,
                                                                &opts,
                                                            )
                                                            .unwrap();
                                                        println!("timecode: {entry:?}");
                                                    }
                                                }
                                                (
                                                    MediaKind::Base {
                                                        header,
                                                        info,
                                                        handler_ref,
                                                        timecode: None,
                                                    },
                                                    Some(subtype),
                                                ) if subtype != MediaComponentSubtype::Timecode => {
                                                    println!("base header: {header:?}");
                                                    println!("base info: {info:?}");
                                                    println!("handler ref: {handler_ref:?}");
                                                    match subtype {
                                                        MediaComponentSubtype::Text => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_text_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("text: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Music => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_music_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("music: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Mpeg => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_mpeg_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("mpeg: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Sprite => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_sprite_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("sprite: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Tween => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_tween_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("tween: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Q3d => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_3d_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("3d: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Streaming => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_streaming_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("streaming: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Hint => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_hint_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("hint: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Qtvr => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_vr_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("qtvr: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Panoramic => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_panoramic_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("panoramic: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Object => {
                                                            while let Some(desc) = desc_iter.next()
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_object_description(
                                                                        &mut desc_iter.reader,
                                                                        &opts,
                                                                    )
                                                                    .unwrap();
                                                                println!("object: {entry:?}");
                                                            }
                                                        }
                                                        o => println!(
                                                            "unknown media component subtype: {o:?}"
                                                        ),
                                                    }
                                                }
                                                o => panic!("bad media format: {o:?}"),
                                            }
                                        }
                                    }
                                    trak::Child::Userdata(udata) => {
                                        for child in udata.children(r, &opts) {
                                            println!("trak udata: {child:?}");
                                        }
                                    }
                                    trak::Child::Unsupported(fcc) => {
                                        println!("unsupported in trak: {fcc:?}")
                                    }
                                }
                            }

                            if let (
                                Some(kind),
                                Some(time_to_sample),
                                Some(sample_size),
                                Some(sample_to_chunk),
                                Some(chunk_offset),
                            ) = (
                                sample_description,
                                time_to_sample,
                                sample_sizes,
                                sample_to_chunk,
                                chunk_offset,
                            ) {
                                tracks.push(PlayableTrack {
                                    kind,
                                    time_to_sample,
                                    sample_size,
                                    sample_to_chunk,
                                    chunk_offset,
                                })
                            }
                        }
                        moov::Child::Userdata(udta) => {
                            for child in udta.children(r, &opts) {
                                println!("moov udata: {child:?}");
                            }
                        }
                        moov::Child::ColorTable(ctb) => {
                            println!("color table: {ctb:?}");
                        }
                        moov::Child::Unsupported(missed) => {
                            println!("skipped: {missed:?}");
                        }
                        moov::Child::CompressedMovie(..) | moov::Child::ReferenceMovie(..) => (),
                    }
                }
            }
            root::Child::MovieData(mdat) => {
                movie_data = Some(mdat);
            }
            root::Child::Unsupported(a) => println!("unsupported {a:?}"),
        }
    }
    drop(root_iter);

    println!("tracks: {tracks:?} (len: {})", tracks.len());
    println!("mdat: {movie_data:?}");

    if let (Some(movie_data), Some(movie_header)) = (movie_data, movie_header) {
        let movie_header = match movie_header.version {
            mvhd::MovieHeaderVersions::V0(mvhd) => mvhd,
            _ => panic!(),
        };

        let duration = movie_header.duration();
        println!("duration: {:?}", duration);

        for track in &tracks {
            // only using the first entry specified for now

            let time_to_sample = match &track.time_to_sample.version {
                stts::TimeToSampleVersions::V0(stts) => stts,
                _ => panic!(),
            };
            let sample_size = match &track.sample_size.version {
                stsz::SampleSizeVersions::V0(stsz) => stsz,
                _ => panic!(),
            };
            let sample_to_chunk = match &track.sample_to_chunk.version {
                stsc::SampleToChunkVersions::V0(stsc) => stsc,
                _ => panic!(),
            };
            let chunk_offset = match &track.chunk_offset.version {
                stco::ChunkOffsetVersions::V0(stco) => stco,
                _ => panic!(),
            };

            match &track.kind {
                TrackKind::Audio(AudioTrack { entries }) => {
                    let mut start_time = 0;
                    let mut frame_idx = 0;
                    let mut time_to_sample_iter =
                        time_to_sample.time_to_sample_table(&mut r, &opts);

                    while let Some(entry) = time_to_sample_iter.next() {
                        let time_to_sample_entry = entry.unwrap();
                        println!("time to sample entry: {time_to_sample_entry:?}");
                        let reader = &mut time_to_sample_iter.reader;
                        for _ in 0..time_to_sample_entry.sample_count {
                            let offset = sample_to_chunk
                                .get_frame_offset(
                                    frame_idx,
                                    &chunk_offset,
                                    &sample_size,
                                    &mut *reader,
                                    &opts,
                                )
                                .unwrap();

                            let time = movie_header.duration_from_step(start_time);
                            println!("offset: {offset:?} @ {time:?}");

                            let frame_data = movie_data.from_offset_32(offset).unwrap();

                            use atom_parser::Reader;
                            let _bytes = reader.read_bytes(frame_data).unwrap();
                            // TODO: send this off to whatever decoder/frame buffer depending on the
                            // codec specified in the sample description

                            start_time += time_to_sample_entry.sample_duration;
                            frame_idx += 1;
                        }
                    }
                    let end = movie_header.duration_from_step(start_time);
                    println!("end time: {end:?}");
                    let _entry = entries.first().unwrap();
                }
                TrackKind::Video(VideoTrack { entries }) => {
                    let mut start_time = 0;
                    let mut frame_idx = 0;
                    let mut time_to_sample_iter =
                        time_to_sample.time_to_sample_table(&mut r, &opts);

                    while let Some(entry) = time_to_sample_iter.next() {
                        let time_to_sample_entry = entry.unwrap();
                        println!("time to sample entry: {time_to_sample_entry:?}");
                        let reader = &mut time_to_sample_iter.reader;
                        for _ in 0..time_to_sample_entry.sample_count {
                            let offset = sample_to_chunk
                                .get_frame_offset(
                                    frame_idx,
                                    &chunk_offset,
                                    &sample_size,
                                    &mut *reader,
                                    &opts,
                                )
                                .unwrap();
                            let time = movie_header.duration_from_step(start_time);
                            println!("offset: {offset:?} @ {time:?}");

                            let frame_data = movie_data.from_offset_32(offset).unwrap();

                            use atom_parser::Reader;
                            let _bytes = reader.read_bytes(frame_data).unwrap();
                            // TODO: send this off to whatever decoder/frame buffer depending on the
                            // codec specified in the sample description

                            start_time += time_to_sample_entry.sample_duration;
                            frame_idx += 1;
                        }
                    }
                    let end = movie_header.duration_from_step(start_time);
                    println!("end time: {end:?}");
                    let _entry = entries.first().unwrap();
                }
            }
        }
    }
    panic!()
}

#[tokio::test]
async fn ftyp_async() {
    let mut r = create_reader();
    let opts = Default::default();

    use atom_parser::AsyncParse;
    let root = Root::parse_async(&mut r, &opts).await.unwrap();
    let mut root_iter = root.children_async(&mut r, &opts);

    let mut tracks = vec![];
    let mut movie_data = None;
    let mut movie_header = None;

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
                            movie_header = Some(hd);
                        }
                        moov::Child::Clipping(clip) => {
                            println!("clip: {clip:?}");
                        }
                        moov::Child::Track(track) => {
                            println!("track: {track:?}");

                            let mut sample_description = None;
                            let mut time_to_sample = None;
                            let mut sample_sizes = None;
                            let mut sample_to_chunk = None;
                            let mut chunk_offset = None;
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
                                                        println!("unknown elst: {v:?}")
                                                    }
                                                },
                                                edts::Child::Unsupported(..) => (),
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
                                        let mut media_info = None;
                                        let mut handler_ref = None;
                                        let mut sample_desc_table = None;

                                        let mut child_iter = m.children_async(r, &opts);
                                        while let Some(child) = child_iter.next().await {
                                            let r = child_iter.reader();
                                            match child.unwrap() {
                                                mdia::Child::MediaHeader(h) => {
                                                    println!("media heaader: {h:?}")
                                                }
                                                mdia::Child::HandlerReference(r) => {
                                                    if handler_ref.as_ref().is_some() {
                                                        panic!("duplicate handler refs");
                                                    }
                                                    match r.version {
                                                        hdlr::HandlerReferenceVersions::V0(v0) => {
                                                            match v0.component_type {
                                                                hdlr::v0::ComponentType::Media(
                                                                    m,
                                                                ) => handler_ref = Some(m.subtype),
                                                                o => panic!(
                                                                    "unknown media type: {o:?}"
                                                                ),
                                                            }
                                                        }
                                                        v => panic!("unknown hdlr version: {v:?}"),
                                                    }
                                                }
                                                mdia::Child::MediaInformation(info) => {
                                                    println!("info: {info:?}");

                                                    let DiscoveredMediaInformation {
                                                        kind,
                                                        data_information,
                                                        sample_table
                                                    } = DiscoveredMediaInformation::from_children_async(&info, &mut *r, &opts).await.unwrap();

                                                    if media_info.is_some() {
                                                        panic!("duplicate media infos");
                                                    }
                                                    media_info = Some(kind);

                                                    if let Some(dinfo) = data_information {
                                                        println!("dinfo: {dinfo:?}");
                                                        let mut child_iter =
                                                            dinfo.children_async(r, &opts);
                                                        while let Some(child) =
                                                            child_iter.next().await
                                                        {
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
                                                                                    dref::v0::Child::Unsupported(..) => (),
                                                                                }
                                                                            }
                                                                        }
                                                                        dref::DataReferenceVersions::Unknown(v) => println!("unknown dref: {v:?}"),
                                                                    }
                                                                }
                                                                dinf::Child::Unsupported(..) => (),
                                                            }
                                                        }
                                                    }

                                                    if let Some(stable) = sample_table {
                                                        println!("stable: {stable:?}");

                                                        let mut child_iter =
                                                            stable.children_async(r, &opts);
                                                        while let Some(child) =
                                                            child_iter.next().await
                                                        {
                                                            let r = child_iter.reader();
                                                            match child.unwrap() {
                                                                stbl::Child::SampleDescription(desc) => {
                                                                    if sample_desc_table.is_some() {
                                                                        panic!("more than 1 stsd atom found in the same minf atom");
                                                                    }
                                                                    sample_desc_table = Some(desc);
                                                                }
                                                                stbl::Child::TimeToSample(tts) => {
                                                                    match &tts.version {
                                                                        stts::TimeToSampleVersions::V0(tts) => {
                                                                            let time_to_sample = tts.time_to_sample_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                            println!("stts entries: {time_to_sample:?}");
                                                                        }
                                                                        stts::TimeToSampleVersions::Unknown(v) => println!("unknown stts: {v:?}"),
                                                                    }
                                                                    time_to_sample = Some(tts);
                                                                }
                                                                stbl::Child::SyncSample(sync) => {
                                                                    match sync.version {
                                                                        stss::SyncSampleVersions::V0(sync) => {
                                                                            println!("sync sample: {sync:?}");
                                                                            let sync_samples = sync.sync_sample_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                            println!("sync sample entries: {sync_samples:?}");
                                                                        }
                                                                        stss::SyncSampleVersions::Unknown(v) => println!("unknown stts: {v:?}"),
                                                                    }
                                                                }
                                                                stbl::Child::SampleToChunk(stc) => {
                                                                    match &stc.version {
                                                                        stsc::SampleToChunkVersions::V0(stc) => {
                                                                            println!("stsc: {stc:?}");
                                                                            let sample_to_chunk = stc.sample_to_chunk_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                            println!("stsc entries: {sample_to_chunk:?}");
                                                                        }
                                                                        stsc::SampleToChunkVersions::Unknown(v) => println!("unknown stsc: {v:?}"),
                                                                    }
                                                                    sample_to_chunk = Some(stc);
                                                                }
                                                                stbl::Child::SampleSize(ss) => {
                                                                    match &ss.version {
                                                                        stsz::SampleSizeVersions::V0(ss) => {
                                                                            println!("stsz: {ss:?}");
                                                                            match &ss.sample_size {
                                                                                stsz::v0::SampleSize::Uniform(u) => println!("uniform: {u:?}"),
                                                                                stsz::v0::SampleSize::Table(st) => {
                                                                                    let sample_sizes = st.sample_size_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                                    println!("stsz entries: {sample_sizes:?}");
                                                                                }
                                                                            }
                                                                        }
                                                                        stsz::SampleSizeVersions::Unknown(v) => println!("unknown stsz: {v:?}"),
                                                                    }
                                                                    sample_sizes = Some(ss);
                                                                }
                                                                stbl::Child::ChunkOffset(co) => {
                                                                    match &co.version {
                                                                        stco::ChunkOffsetVersions::V0(co) => {
                                                                            println!("co32: {co:?}");
                                                                            let offsets = co.chunk_offset_table_async(r, &opts).collect::<Result<Vec<_>, _>>().await.unwrap();
                                                                            println!("co32 entries: {offsets:?}")
                                                                        }
                                                                        stco::ChunkOffsetVersions::Unknown(v) => println!("unknown stco: {v:?}"),
                                                                    }
                                                                    chunk_offset = Some(co);
                                                                }
                                                                stbl::Child::Unsupported(u) => println!("unsupported stbl entry: {u:?}"),
                                                            }
                                                        }
                                                    }
                                                }
                                                mdia::Child::Userdata(u) => {
                                                    let mut child_iter = u.children_async(r, &opts);
                                                    while let Some(child) = child_iter.next().await
                                                    {
                                                        println!("mdia udata: {child:?}");
                                                    }
                                                }
                                                mdia::Child::Unsupported(_) => (),
                                            }
                                        }
                                        drop(child_iter);

                                        if let (Some(media_info), Some(sample_desc_table)) =
                                            (media_info, sample_desc_table)
                                        {
                                            let stsd = match sample_desc_table.version {
                                                stsd::SampleDescriptionVersions::V0(stsd) => stsd,
                                                v => panic!("unknown stsd version: {v:?}"),
                                            };
                                            let mut desc_iter = stsd.descriptions_async(r, &opts);
                                            match (media_info, handler_ref) {
                                                (
                                                    MediaKind::Video {
                                                        header,
                                                        handler_ref,
                                                    },
                                                    Some(MediaComponentSubtype::Video) | None,
                                                ) => {
                                                    println!("video header: {header:?}");
                                                    println!("video handler ref: {handler_ref:?}");
                                                    let mut entries = vec![];
                                                    while let Some(desc) = desc_iter.next().await {
                                                        let desc = desc.unwrap();
                                                        let entry = desc
                                                            .content
                                                            .as_video_entry_async(
                                                                desc.fourcc,
                                                                desc_iter.reader(),
                                                                &opts,
                                                            )
                                                            .await
                                                            .unwrap();
                                                        println!("video: {entry:?}");
                                                        let mut extension_iter =
                                                            entry.entry.description.children_async(
                                                                desc_iter.reader(),
                                                                &opts,
                                                            );

                                                        while let Some(extension) =
                                                            extension_iter.next().await
                                                        {
                                                            let extension = extension.unwrap();
                                                            println!(
                                                                "video extension: {extension:?}"
                                                            );
                                                        }
                                                        entries.push(entry);
                                                    }
                                                    sample_description =
                                                        Some(TrackKind::Video(VideoTrack {
                                                            entries,
                                                        }));
                                                }
                                                (
                                                    MediaKind::Audio {
                                                        header,
                                                        handler_ref,
                                                    },
                                                    Some(MediaComponentSubtype::Audio) | None,
                                                ) => {
                                                    println!("audio header: {header:?}");
                                                    println!("audio handler ref: {handler_ref:?}");
                                                    let mut entries = vec![];
                                                    while let Some(desc) = desc_iter.next().await {
                                                        let desc = desc.unwrap();
                                                        let entry = desc
                                                            .content
                                                            .as_audio_entry_async(
                                                                desc.fourcc,
                                                                desc_iter.reader(),
                                                                &opts,
                                                            )
                                                            .await
                                                            .unwrap();
                                                        entries.push(entry);
                                                    }
                                                    sample_description =
                                                        Some(TrackKind::Audio(AudioTrack {
                                                            entries,
                                                        }));
                                                }
                                                (
                                                    MediaKind::Base {
                                                        header,
                                                        info,
                                                        handler_ref,
                                                        timecode: Some(timecode),
                                                    },
                                                    Some(MediaComponentSubtype::Timecode),
                                                ) => {
                                                    println!("timecode base header: {header:?}");
                                                    println!("timecode base info: {info:?}");
                                                    println!(
                                                        "timecode handler ref: {handler_ref:?}"
                                                    );
                                                    println!("timecode info: {timecode:?}");
                                                    while let Some(desc) = desc_iter.next().await {
                                                        let entry = desc
                                                            .unwrap()
                                                            .content
                                                            .as_timecode_description_async(
                                                                desc_iter.reader(),
                                                                &opts,
                                                            )
                                                            .await
                                                            .unwrap();
                                                        println!("timecode: {entry:?}");
                                                    }
                                                }
                                                (
                                                    MediaKind::Base {
                                                        header,
                                                        info,
                                                        handler_ref,
                                                        timecode: None,
                                                    },
                                                    Some(subtype),
                                                ) if subtype != MediaComponentSubtype::Timecode => {
                                                    println!("base header: {header:?}");
                                                    println!("base info: {info:?}");
                                                    println!("handler ref: {handler_ref:?}");
                                                    match subtype {
                                                        MediaComponentSubtype::Text => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_text_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("text: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Music => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_music_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("music: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Mpeg => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_mpeg_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("mpeg: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Sprite => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_sprite_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("sprite: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Tween => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_tween_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("tween: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Q3d => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_3d_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("3d: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Streaming => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_streaming_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("streaming: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Hint => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_hint_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("hint: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Qtvr => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_vr_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("qtvr: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Panoramic => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_panoramic_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("panoramic: {entry:?}");
                                                            }
                                                        }
                                                        MediaComponentSubtype::Object => {
                                                            while let Some(desc) =
                                                                desc_iter.next().await
                                                            {
                                                                let entry = desc
                                                                    .unwrap()
                                                                    .content
                                                                    .as_object_description_async(
                                                                        desc_iter.reader(),
                                                                        &opts,
                                                                    )
                                                                    .await
                                                                    .unwrap();
                                                                println!("object: {entry:?}");
                                                            }
                                                        }
                                                        o => println!(
                                                            "unknown media component subtype: {o:?}"
                                                        ),
                                                    }
                                                }
                                                o => panic!("bad media format: {o:?}"),
                                            }
                                        }
                                    }
                                    trak::Child::Userdata(udata) => {
                                        let mut child_iter = udata.children_async(r, &opts);
                                        while let Some(child) = child_iter.next().await {
                                            println!("trak udata: {child:?}");
                                        }
                                    }
                                    trak::Child::Unsupported(fcc) => {
                                        println!("unsupported in trak: {fcc:?}")
                                    }
                                }
                            }

                            if let (
                                Some(kind),
                                Some(time_to_sample),
                                Some(sample_size),
                                Some(sample_to_chunk),
                                Some(chunk_offset),
                            ) = (
                                sample_description,
                                time_to_sample,
                                sample_sizes,
                                sample_to_chunk,
                                chunk_offset,
                            ) {
                                tracks.push(PlayableTrack {
                                    kind,
                                    time_to_sample,
                                    sample_size,
                                    sample_to_chunk,
                                    chunk_offset,
                                })
                            }
                        }
                        moov::Child::Userdata(udta) => {
                            let mut child_iter = udta.children_async(r, &opts);
                            while let Some(child) = child_iter.next().await {
                                println!("moov udata: {child:?}");
                            }
                        }
                        moov::Child::ColorTable(ctb) => {
                            println!("color table: {ctb:?}");
                        }
                        moov::Child::Unsupported(missed) => {
                            println!("skipped: {missed:?}");
                        }
                        moov::Child::CompressedMovie(..) | moov::Child::ReferenceMovie(..) => (),
                    }
                }
            }
            root::Child::MovieData(mdat) => {
                movie_data = Some(mdat);
            }
            root::Child::Unsupported(a) => println!("unsupported {a:?}"),
        }
    }
    drop(root_iter);

    println!("tracks: {tracks:?} (len: {})", tracks.len());
    println!("mdat: {movie_data:?}");

    if let (Some(movie_data), Some(movie_header)) = (movie_data, movie_header) {
        let movie_header = match movie_header.version {
            mvhd::MovieHeaderVersions::V0(mvhd) => mvhd,
            _ => panic!(),
        };

        let duration = movie_header.duration();
        println!("duration: {:?}", duration);

        for track in &tracks {
            // only using the first entry specified for now

            let time_to_sample = match &track.time_to_sample.version {
                stts::TimeToSampleVersions::V0(stts) => stts,
                _ => panic!(),
            };
            let sample_size = match &track.sample_size.version {
                stsz::SampleSizeVersions::V0(stsz) => stsz,
                _ => panic!(),
            };
            let sample_to_chunk = match &track.sample_to_chunk.version {
                stsc::SampleToChunkVersions::V0(stsc) => stsc,
                _ => panic!(),
            };
            let chunk_offset = match &track.chunk_offset.version {
                stco::ChunkOffsetVersions::V0(stco) => stco,
                _ => panic!(),
            };

            match &track.kind {
                TrackKind::Audio(AudioTrack { entries }) => {
                    let mut start_time = 0;
                    let mut frame_idx = 0;
                    let mut time_to_sample_iter =
                        time_to_sample.time_to_sample_table_async(&mut r, &opts);

                    while let Some(entry) = time_to_sample_iter.next().await {
                        let time_to_sample_entry = entry.unwrap();
                        println!("time to sample entry: {time_to_sample_entry:?}");
                        let reader = time_to_sample_iter.reader();
                        for _ in 0..time_to_sample_entry.sample_count {
                            let offset = sample_to_chunk
                                .get_frame_offset_async(
                                    frame_idx,
                                    &chunk_offset,
                                    &sample_size,
                                    &mut *reader,
                                    &opts,
                                )
                                .await
                                .unwrap();

                            let time = movie_header.duration_from_step(start_time);
                            println!("offset: {offset:?} @ {time:?}");

                            let frame_data = movie_data.from_offset_32(offset).unwrap();

                            use atom_parser::AsyncReader;
                            let _bytes = reader.async_read_bytes(frame_data).await.unwrap();
                            // TODO: send this off to whatever decoder/frame buffer depending on the
                            // codec specified in the sample description

                            start_time += time_to_sample_entry.sample_duration;
                            frame_idx += 1;
                        }
                    }
                    let end = movie_header.duration_from_step(start_time);
                    println!("end time: {end:?}");
                    let _entry = entries.first().unwrap();
                }
                TrackKind::Video(VideoTrack { entries }) => {
                    let mut start_time = 0;
                    let mut frame_idx = 0;
                    let mut time_to_sample_iter =
                        time_to_sample.time_to_sample_table_async(&mut r, &opts);

                    while let Some(entry) = time_to_sample_iter.next().await {
                        let time_to_sample_entry = entry.unwrap();
                        println!("time to sample entry: {time_to_sample_entry:?}");
                        let reader = time_to_sample_iter.reader();
                        for _ in 0..time_to_sample_entry.sample_count {
                            let offset = sample_to_chunk
                                .get_frame_offset_async(
                                    frame_idx,
                                    &chunk_offset,
                                    &sample_size,
                                    &mut *reader,
                                    &opts,
                                )
                                .await
                                .unwrap();
                            let time = movie_header.duration_from_step(start_time);
                            println!("offset: {offset:?} @ {time:?}");

                            let frame_data = movie_data.from_offset_32(offset).unwrap();

                            use atom_parser::AsyncReader;
                            let _bytes = reader.async_read_bytes(frame_data).await.unwrap();
                            // TODO: send this off to whatever decoder/frame buffer depending on the
                            // codec specified in the sample description

                            start_time += time_to_sample_entry.sample_duration;
                            frame_idx += 1;
                        }
                    }
                    let end = movie_header.duration_from_step(start_time);
                    println!("end time: {end:?}");
                    let _entry = entries.first().unwrap();
                }
            }
        }
    }
    panic!()
}
