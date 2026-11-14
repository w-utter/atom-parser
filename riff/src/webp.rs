use atom_parser::{FourCC, Payload, make_atom};

impl crate::RiffDerived for Entry {
    const FILE_TYPE: FourCC = FourCC::new(*b"WEBP");
}

pub type WebpFile = crate::RiffDerivedHeader<Entry>;

make_atom! {
    #[derive(Debug)]
    struct Entry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        entry_kind: enum Entry {
            LossyData,
            LosslessData,
            Alpha,

            ExtendedInfo,

            Animation,
            AnimationFrame,

            IccProfile,

            ExifMetadata,
            XmpMetadata,
        }
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"VP8 "))]
    struct LossyData {
        #[payload]
        lossy_data: Vec<u8>,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"VP8L"))]
    struct LosslessData {
        #[payload]
        lossless_data: Vec<u8>,
    }
}

make_atom! {
    #[bitfield_repr(u64)]
    struct ExtendedInfoPacked {
        #[reserved]
        #[bits(16)]
        reserved: u16,
        #[bits(24)]
        width_minus_one: u32,
        #[bits(24)]
        height_minus_one: u32,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"VP8X"))]
    struct ExtendedInfo {
        #[flags(u16)]
        flags: struct InfoFlags {
            const ANIMATION = 1 << 1;
            const XMP_METADATA = 1 << 2;
            const EXIF_METADATA = 1 << 3;
            const ALPHA = 1 << 4;
            const ICC_PROFILE = 1 << 5;
        },
        info: ExtendedInfoPacked,
    }
}

impl<O> ExtendedInfo<O> {
    pub fn width(&self) -> u32 {
        self.info.width_minus_one() + 1
    }

    pub fn height(&self) -> u32 {
        self.info.height_minus_one() + 1
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"ANIM"))]
    struct Animation {
        background_rgba: [u8; 4],
        loop_count: u16,
    }

    #[bitfield_repr(u128)]
    struct AnimationFramePacked {
        #[bits(24)]
        frame_x: u32,
        #[bits(24)]
        frame_y: u32,
        #[bits(24)]
        frame_width_minus_one: u32,
        #[bits(24)]
        frame_height_minus_one: u32,
        #[bits(24)]
        frame_duration_ms: u32,
        #[reserved]
        #[bits(6)]
        reserved: u8,
        #[flag]
        do_not_blend: bool,
        #[flag]
        dispose: bool,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"ANMF"))]
    struct AnimationFrame {
        packed: AnimationFramePacked,
        #[trailing_array]
        data: AnimationFrameData,
    }

    #[derive(Debug)]
    struct AnimationFrameData {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        entry_kind: enum Entry {
            LossyData,
            LosslessData,
            Alpha,
        }
    }
}

impl<O> AnimationFrame<O> {
    pub fn x(&self) -> u32 {
        self.packed.frame_x() * 2
    }

    pub fn y(&self) -> u32 {
        self.packed.frame_y() * 2
    }

    pub fn width(&self) -> u32 {
        self.packed.frame_width_minus_one() + 1
    }

    pub fn height(&self) -> u32 {
        self.packed.frame_height_minus_one() + 1
    }
}

make_atom! {
    #[bitfield_repr(u8)]
    struct AlphaPacked {
        #[reserved]
        #[bits(2)]
        reserved: u8,
        #[flag]
        no_preprocessing: bool,
        #[flag]
        level_reduction: bool,
        #[enum_bits(2)]
        filtering_method: enum FilteringMethod {
            None = 0,
            Horizontal = 1,
            Vertical = 2,
            Gradient = 3,
        },
        #[flag]
        webp_lossless_compression: bool,
        #[reserved]
        #[bits(1)]
        reserved: u8,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"ALPH"))]
    struct Alpha {
        packed: AlphaPacked,
        #[payload]
        bitstream: Vec<u8>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"ICCP"))]
    struct IccProfile {
        #[payload]
        icc_profile: Vec<u8>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"EXIF"))]
    struct ExifMetadata {
        #[payload]
        metadata: Vec<u8>
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"XMP "))]
    struct XmpMetadata {
        #[payload]
        metadata: Vec<u8>
    }
}

#[derive(Debug)]
pub enum FrameKind<O> {
    Lossy(Payload<O>),
    Lossless(Payload<O>),
}

#[derive(Debug)]
pub struct Frame<O> {
    pub kind: FrameKind<O>,
    pub alpha: Option<Alpha<O>>,
}

impl<O> Frame<O> {
    pub fn from_data(
        kind: &mut Option<FrameKind<O>>,
        alpha: &mut Option<Alpha<O>>,
    ) -> Option<Self> {
        let kind = core::mem::take(kind)?;
        let alpha = core::mem::take(alpha);
        Some(Self { kind, alpha })
    }
}

#[cfg(test)]
const TEST_FILE: &str = "../webp_test_2.webp";

#[test]
fn webp() {
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
    let webp = WebpFile::parse(&mut r, &opts).unwrap();
    println!("webp: {webp:?}");

    let mut entry_iter = webp.entries(&mut r, &opts).unwrap();

    let mut extended_info: Option<ExtendedInfo<_>> = None;
    let mut exif_mdta = None;
    let mut xmp_mdta = None;
    let mut icc_profile = None;
    let mut animation_info = None;

    let mut frame_kind = None;
    let mut frame_alpha = None;

    let should_ignore_data_entries = |info: Option<&ExtendedInfo<_>>| {
        use extended_info::InfoFlags;
        if let Some(info) = info.as_ref() {
            info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION
        } else {
            false
        }
    };

    while let Some(entry) = entry_iter.next() {
        let entry = entry.unwrap();
        let reader = &mut entry_iter.reader();
        use entry::Entry;
        use extended_info::InfoFlags;
        match (entry.entry_kind, extended_info.as_ref()) {
            (Entry::ExtendedInfo(info), None) => {
                extended_info = Some(info);
            }
            (Entry::ExtendedInfo(_), Some(_)) => panic!("multiple format chunks"),
            (Entry::ExifMetadata(exif), Some(info))
                if exif_mdta.is_none()
                    && info.flags & InfoFlags::EXIF_METADATA == InfoFlags::EXIF_METADATA =>
            {
                exif_mdta = Some(exif);
            }
            (Entry::XmpMetadata(xmp), Some(info))
                if xmp_mdta.is_none()
                    && info.flags & InfoFlags::XMP_METADATA == InfoFlags::XMP_METADATA =>
            {
                xmp_mdta = Some(xmp);
            }
            (Entry::IccProfile(icc), Some(info))
                if icc_profile.is_none()
                    && info.flags & InfoFlags::ICC_PROFILE == InfoFlags::ICC_PROFILE =>
            {
                icc_profile = Some(icc)
            }
            (Entry::Animation(animation), Some(info))
                if animation_info.is_none()
                    && info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION =>
            {
                animation_info = Some(animation)
            }
            (Entry::AnimationFrame(frame), Some(info))
                if info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION =>
            {
                assert!(
                    frame_kind.is_none() && frame_alpha.is_none(),
                    "frame data should always be cleared out before going into an animation frame"
                );

                let mut data_iter = frame.data(reader, &opts).unwrap();
                while let Some(data) = data_iter.next() {
                    let data = data.unwrap();
                    use animation_frame_data::Entry;
                    match data.entry_kind {
                        Entry::LossyData(lossy) if frame_kind.is_none() => {
                            frame_kind = Some(FrameKind::Lossy(lossy.lossy_data));
                        }
                        Entry::LosslessData(lossless) if frame_kind.is_none() => {
                            frame_kind = Some(FrameKind::Lossless(lossless.lossless_data))
                        }
                        Entry::Alpha(alpha)
                            if info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION =>
                        {
                            frame_alpha = Some(alpha)
                        }
                        _ => (),
                    }
                }
                let info = frame.packed;
                let frame = Frame::from_data(&mut frame_kind, &mut frame_alpha)
                    .expect("no frame data found");
                println!("animated frame: {frame:?} with info {info:?}");
            }
            (Entry::LossyData(_) | Entry::LosslessData(_) | Entry::Alpha(_), info)
                if should_ignore_data_entries(info) =>
            {
                continue;
            }
            (Entry::LossyData(lossy), _) if frame_kind.is_none() => {
                frame_kind = Some(FrameKind::Lossy(lossy.lossy_data))
            }
            (Entry::LosslessData(lossless), _) if frame_kind.is_none() => {
                frame_kind = Some(FrameKind::Lossless(lossless.lossless_data))
            }
            (Entry::Alpha(alpha), Some(info))
                if frame_alpha.is_none() && info.flags & InfoFlags::ALPHA == InfoFlags::ALPHA =>
            {
                frame_alpha = Some(alpha)
            }
            _ => (),
        }
    }

    if !should_ignore_data_entries(extended_info.as_ref()) {
        let frame =
            Frame::from_data(&mut frame_kind, &mut frame_alpha).expect("no frame data found");
        println!("frame: {frame:?}");
    }

    if let Some(info) = extended_info.as_ref() {
        println!("extended info: {info:?}");
    }

    if let Some(exif) = exif_mdta.as_ref() {
        println!("exif: {exif:?}");
    }

    if let Some(xmp) = xmp_mdta.as_ref() {
        println!("xmp: {xmp:?}");
    }

    if let Some(icc) = icc_profile.as_ref() {
        println!("icc profile: {icc:?}");
    }
    panic!()
}

#[tokio::test]
async fn webp_async() {
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
    let webp = WebpFile::parse_async(&mut r, &opts).await.unwrap();
    println!("webp: {webp:?}");

    let mut entry_iter = webp.entries_async(&mut r, &opts).unwrap();

    let mut extended_info: Option<ExtendedInfo<_>> = None;
    let mut exif_mdta = None;
    let mut xmp_mdta = None;
    let mut icc_profile = None;
    let mut animation_info = None;

    let mut frame_kind = None;
    let mut frame_alpha = None;

    let should_ignore_data_entries = |info: Option<&ExtendedInfo<_>>| {
        use extended_info::InfoFlags;
        if let Some(info) = info.as_ref() {
            info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION
        } else {
            false
        }
    };

    use tokio_stream::StreamExt;
    while let Some(entry) = entry_iter.next().await {
        let entry = entry.unwrap();
        let reader = &mut entry_iter.reader();
        use entry::Entry;
        use extended_info::InfoFlags;
        match (entry.entry_kind, extended_info.as_ref()) {
            (Entry::ExtendedInfo(info), None) => {
                extended_info = Some(info);
            }
            (Entry::ExtendedInfo(_), Some(_)) => panic!("multiple format chunks"),
            (Entry::ExifMetadata(exif), Some(info))
                if exif_mdta.is_none()
                    && info.flags & InfoFlags::EXIF_METADATA == InfoFlags::EXIF_METADATA =>
            {
                exif_mdta = Some(exif);
            }
            (Entry::XmpMetadata(xmp), Some(info))
                if xmp_mdta.is_none()
                    && info.flags & InfoFlags::XMP_METADATA == InfoFlags::XMP_METADATA =>
            {
                xmp_mdta = Some(xmp);
            }
            (Entry::IccProfile(icc), Some(info))
                if icc_profile.is_none()
                    && info.flags & InfoFlags::ICC_PROFILE == InfoFlags::ICC_PROFILE =>
            {
                icc_profile = Some(icc)
            }
            (Entry::Animation(animation), Some(info))
                if animation_info.is_none()
                    && info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION =>
            {
                animation_info = Some(animation)
            }
            (Entry::AnimationFrame(frame), Some(info))
                if info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION =>
            {
                assert!(
                    frame_kind.is_none() && frame_alpha.is_none(),
                    "frame data should always be cleared out before going into an animation frame"
                );

                let mut data_iter = frame.data_async(reader, &opts).unwrap();
                while let Some(data) = data_iter.next().await {
                    let data = data.unwrap();
                    use animation_frame_data::Entry;
                    match data.entry_kind {
                        Entry::LossyData(lossy) if frame_kind.is_none() => {
                            frame_kind = Some(FrameKind::Lossy(lossy.lossy_data));
                        }
                        Entry::LosslessData(lossless) if frame_kind.is_none() => {
                            frame_kind = Some(FrameKind::Lossless(lossless.lossless_data))
                        }
                        Entry::Alpha(alpha)
                            if info.flags & InfoFlags::ANIMATION == InfoFlags::ANIMATION =>
                        {
                            frame_alpha = Some(alpha)
                        }
                        _ => (),
                    }
                }
                let info = frame.packed;
                let frame = Frame::from_data(&mut frame_kind, &mut frame_alpha)
                    .expect("no frame data found");
                println!("animated frame: {frame:?} with info {info:?}");
            }
            (Entry::LossyData(_) | Entry::LosslessData(_) | Entry::Alpha(_), info)
                if should_ignore_data_entries(info) =>
            {
                continue;
            }
            (Entry::LossyData(lossy), _) if frame_kind.is_none() => {
                frame_kind = Some(FrameKind::Lossy(lossy.lossy_data))
            }
            (Entry::LosslessData(lossless), _) if frame_kind.is_none() => {
                frame_kind = Some(FrameKind::Lossless(lossless.lossless_data))
            }
            (Entry::Alpha(alpha), Some(info))
                if frame_alpha.is_none() && info.flags & InfoFlags::ALPHA == InfoFlags::ALPHA =>
            {
                frame_alpha = Some(alpha)
            }
            _ => (),
        }
    }

    if !should_ignore_data_entries(extended_info.as_ref()) {
        let frame =
            Frame::from_data(&mut frame_kind, &mut frame_alpha).expect("no frame data found");
        println!("frame: {frame:?}");
    }

    if let Some(info) = extended_info.as_ref() {
        println!("extended info: {info:?}");
    }

    if let Some(exif) = exif_mdta.as_ref() {
        println!("exif: {exif:?}");
    }

    if let Some(xmp) = xmp_mdta.as_ref() {
        println!("xmp: {xmp:?}");
    }

    if let Some(icc) = icc_profile.as_ref() {
        println!("icc profile: {icc:?}");
    }
    panic!()
}
