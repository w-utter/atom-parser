use atom_parser::{FourCC, make_atom};

#[cfg(feature = "error_detection")]
use atom_parser::FullHasher;

#[cfg(feature = "error_detection")]
#[derive(Default)]
struct PngHasher(crc32fast::Hasher);

#[cfg(feature = "error_detection")]
impl FullHasher for PngHasher {
    type Hash = u32;

    fn finish(self) -> Self::Hash {
        self.0.finalize()
    }
    fn hash(&mut self, bytes: &[u8]) {
        self.0.update(bytes)
    }
}

make_atom! {
    #[endianess(big)]
    struct PngFile {
        #[expected([137, 80, 78, 71, 13, 10, 26, 10])]
        header: [u8; 8],
        #[trailing_array]
        chunks: Chunk,
    }
}

make_atom! {
    #[derive(Debug)]
    #[endianess(big)]
    struct Chunk {
        #[skip_error_detection]
        #[payload_length]
        length: u32,
        #[tag]
        chunk_id: FourCC,
        #[tagged]
        chunk: enum Chunk {
            // required chunks
            ImageHeader,
            Palette,
            ImageData,
            ImageEnd,
            // ancillary
            Transparency,
            ColorSpaceInfo,
            Gamma,
            IccProfile,
            SignificantBits,
            SrgbColorSpace,
            CodingIndependentCodePoints,
            DisplayColorVolume,
            ContentLightLevel,
            HDRReferenceWhiteLevel,
            // text
            Text,
            CompressedText,
            InternationalText,
            // misc
            BackgroundColor,
            ImageHistogram,
            PhysicalPixelDimensions,
            SuggestedPalette,
            ExifProfile,
            // timestamp
            TimestampInfo,
            // animation
            AnimationInfo,
            FrameControl,
            FrameData,
        },
        #[error_detection(PngHasher)]
        crc: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[enum_repr(u8)]
    enum BitDepth {
        One = 1,
        Two = 2,
        Four = 4,
        Eight = 8,
        Sixteen = 16,
    }

    #[derive(Debug)]
    #[enum_repr(u8)]
    enum ColorType {
        Greyscale = 0,
        Rgb = 2,
        Indexed = 3,
        GreyscaleWithAlpha = 4,
        Rgba = 6,
    }

    #[derive(Debug)]
    #[enum_repr(u8)]
    enum InterlaceMethod {
        None = 0,
        Adam7 = 1,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"IHDR"))]
    struct ImageHeader {
        width: u32,
        height: u32,
        bit_depth: BitDepth,
        color_type: ColorType,
        #[expected(0)]
        filter_method: u8,
        interlace_method: InterlaceMethod,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"PLTE"))]
    struct Palette {
        #[trailing_array]
        palette_entries: PaletteEntry,
    }

    #[derive(Debug)]
    struct PaletteEntry {
        rgb: [u8; 3],
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"IDAT"))]
    struct ImageData {
        #[payload]
        data: Vec<u8>,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"IEND"))]
    struct ImageEnd { }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"tRNS"))]
    struct Transparency {
        // dependent on the color type
        #[payload]
        bytes: Vec<u8>
    }
}

pub enum TransparencyInfo<O> {
    Greyscale {
        greyscale_sample: u16,
    },
    Rgb {
        red_sample: u16,
        blue_sample: u16,
        green_sample: u16,
    },
    Indexed {
        alpha_palette_indices: atom_parser::Trailing<O, u8>,
    },
}

impl<O: atom_parser::Offset> TransparencyInfo<O> {
    pub fn parse_from_color_type<R: atom_parser::Reader<O>>(
        ty: &ColorType,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        use atom_parser::Parse;
        Ok(match ty {
            ColorType::Greyscale => {
                let greyscale_sample = u16::parse(reader, opts)?;
                Self::Greyscale { greyscale_sample }
            }
            ColorType::Rgb => {
                let [red_sample, blue_sample, green_sample] = <[u16; 3]>::parse(reader, opts)?;
                Self::Rgb {
                    red_sample,
                    blue_sample,
                    green_sample,
                }
            }
            ColorType::Indexed => {
                let alpha_palette_indices = <atom_parser::Trailing<O, u8>>::parse(reader, opts)?;
                Self::Indexed {
                    alpha_palette_indices,
                }
            }
            o => panic!("transparency chunk unsupported for color type: {o:?}"),
        })
    }

    pub async fn parse_from_color_type_async<R: atom_parser::AsyncReader<O> + Unpin>(
        ty: &ColorType,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError>
    where
        O: Unpin,
    {
        use atom_parser::AsyncParse;
        Ok(match ty {
            ColorType::Greyscale => {
                let greyscale_sample = u16::parse_async(reader, opts).await?;
                Self::Greyscale { greyscale_sample }
            }
            ColorType::Rgb => {
                let [red_sample, blue_sample, green_sample] =
                    <[u16; 3]>::parse_async(reader, opts).await?;
                Self::Rgb {
                    red_sample,
                    blue_sample,
                    green_sample,
                }
            }
            ColorType::Indexed => {
                let alpha_palette_indices =
                    <atom_parser::Trailing<O, u8>>::create_fut(reader, opts).await?;
                Self::Indexed {
                    alpha_palette_indices,
                }
            }
            o => panic!("transparency chunk unsupported for color type: {o:?}"),
        })
    }
}

make_atom! {
    struct Point<T> {
        x: T,
        y: T,
    }
}

impl<T: atom_parser::Parsed, O> core::fmt::Debug for Point<T, O>
where
    <T as atom_parser::Parsed>::Output<O>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Point")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"cHRM"))]
    struct ColorSpaceInfo {
        white_point: Point<u32>,
        red: Point<u32>,
        green: Point<u32>,
        blue: Point<u32>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"gAMA"))]
    struct Gamma {
        gamma: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"iCCP"))]
    struct IccProfile {
        #[null_terminated_string]
        profile_name: String,
        #[expected(0)]
        compression_method: u8,
        #[payload]
        profile: Vec<u8>,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"sBIT"))]
    struct SignificantBits {
        // dependent on the color type
        #[payload]
        bytes: Vec<u8>,
    }
}

pub enum SignificantBitsInfo {
    Greyscale { greyscale: u8 },
    Rgb { r: u8, g: u8, b: u8 },
    GreyscaleWithAlpha { greyscale: u8, alpha: u8 },
    Rgba { r: u8, g: u8, b: u8, a: u8 },
}

impl SignificantBitsInfo {
    pub fn parse_from_color_type<O: atom_parser::Offset, R: atom_parser::Reader<O>>(
        ty: &ColorType,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        use atom_parser::Parse;
        Ok(match ty {
            ColorType::Greyscale => {
                let greyscale = u8::parse(reader, opts)?;
                Self::Greyscale { greyscale }
            }
            ColorType::Rgb | ColorType::Indexed => {
                let [r, g, b] = <[u8; 3]>::parse(reader, opts)?;
                Self::Rgb { r, g, b }
            }
            ColorType::GreyscaleWithAlpha => {
                let [greyscale, alpha] = <[u8; 2]>::parse(reader, opts)?;
                Self::GreyscaleWithAlpha { greyscale, alpha }
            }
            ColorType::Rgba => {
                let [r, g, b, a] = <[u8; 4]>::parse(reader, opts)?;
                Self::Rgba { r, g, b, a }
            }
            ColorType::Unknown(o) => panic!("unknown color type: {o:?}"),
        })
    }

    pub async fn parse_from_color_type_async<
        O: atom_parser::Offset + Unpin,
        R: atom_parser::AsyncReader<O> + Unpin,
    >(
        ty: &ColorType,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        use atom_parser::AsyncParse;
        Ok(match ty {
            ColorType::Greyscale => {
                let greyscale = u8::parse_async(reader, opts).await?;
                Self::Greyscale { greyscale }
            }
            ColorType::Rgb | ColorType::Indexed => {
                let [r, g, b] = <[u8; 3]>::parse_async(reader, opts).await?;
                Self::Rgb { r, g, b }
            }
            ColorType::GreyscaleWithAlpha => {
                let [greyscale, alpha] = <[u8; 2]>::parse_async(reader, opts).await?;
                Self::GreyscaleWithAlpha { greyscale, alpha }
            }
            ColorType::Rgba => {
                let [r, g, b, a] = <[u8; 4]>::parse_async(reader, opts).await?;
                Self::Rgba { r, g, b, a }
            }
            ColorType::Unknown(o) => panic!("unknown color type: {o:?}"),
        })
    }
}

make_atom! {
    #[derive(Debug)]
    #[enum_repr(u8)]
    enum RenderingIntent {
        Perceptual = 0,
        RelativeColorimetric = 1,
        Saturation = 2,
        AbsoluteColorimetric = 3,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"sRGB"))]
    struct SrgbColorSpace {
        rendering_intent: RenderingIntent,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"cICP"))]
    struct CodingIndependentCodePoints {
        color_primaries: u8,
        transfer_function: u8,
        #[expected(0)]
        matrix_coefficients: u8,
        video_full_range: bool,
    }
}

make_atom! {
    #[derive(Debug)]
    struct ColorPrimaryChromaticities {
        red: Point<u16>,
        green: Point<u16>,
        blue: Point<u16>,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"mDCV"))]
    struct DisplayColorVolume {
        color_primary_chromaticities: ColorPrimaryChromaticities,
        white_point_chromaticity: Point<u16>,
        maximum_luminance: u32,
        minimum_luminance: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"cLLI"))]
    struct ContentLightLevel {
        maximum_content_light_level: u32,
        maximum_frame_average_light_level: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"hRWL"))]
    struct HDRReferenceWhiteLevel {
        reference_white_level: u32,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"tEXt"))]
    struct Text {
        #[null_terminated_string]
        keyword: String,
        #[payload]
        text: String,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"zTXt"))]
    struct CompressedText {
        #[null_terminated_string]
        keyword: String,
        #[expected(0)]
        compression_method: u8,
        #[payload]
        compressed_text: Vec<u8>,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"iTXt"))]
    struct InternationalText {
        #[null_terminated_string]
        keyword: String,
        compressed: bool,
        #[expected(0)]
        compression_method: u8,
        #[null_terminated_string]
        language_tag: String,
        #[null_terminated_string]
        translated_keyword: String,
        #[payload]
        text: String,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"bKGD"))]
    struct BackgroundColor {
        // dependent on color type
        #[payload]
        bytes: Vec<u8>,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"hIST"))]
    struct ImageHistogram {
        #[trailing_array]
        frequencies: u16,
    }

    #[derive(Debug)]
    #[enum_repr(u8)]
    enum UnitSpecifier {
        UnknownUnit = 0,
        Meter = 1,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"pHYs"))]
    struct PhysicalPixelDimensions {
        pixels_per_unit_x: u32,
        pixels_per_unit_y: u32,
        unit_specifier: UnitSpecifier,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"sPLT"))]
    struct SuggestedPalette {
        #[null_terminated_string]
        palette_name: String,
        #[tag]
        sample_depth: u8,
        #[tagged]
        samples: enum Samples {
            SuggestedPaletteEntries8,
            SuggestedPaletteEntries16,
        }
    }

    #[derive(Debug)]
    #[tag(u8, 8)]
    struct SuggestedPaletteEntries8 {
        #[trailing_array]
        entries: SuggestedPaletteEntry<u8>,
    }

    #[derive(Debug)]
    #[tag(u8, 16)]
    struct SuggestedPaletteEntries16 {
        #[trailing_array]
        entries: SuggestedPaletteEntry<u16>,
    }

    struct SuggestedPaletteEntry<T> {
        red: T,
        green: T,
        blue: T,
        alpha: T,
        frequency: u16,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"eXIf"))]
    struct ExifProfile {
        #[payload]
        profile: Vec<u8>,
    }
}

pub enum BackgroundColorInfo {
    Greyscale { greyscale: u16 },
    Rgb { r: u16, g: u16, b: u16 },
    PaletteIndex { index: u8 },
}

impl BackgroundColorInfo {
    pub fn parse_from_color_type<O: atom_parser::Offset, R: atom_parser::Reader<O>>(
        ty: &ColorType,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        use atom_parser::Parse;
        Ok(match ty {
            ColorType::Greyscale | ColorType::GreyscaleWithAlpha => {
                let greyscale = u16::parse(reader, opts)?;
                Self::Greyscale { greyscale }
            }
            ColorType::Rgb | ColorType::Rgba => {
                let [r, g, b] = <[u16; 3]>::parse(reader, opts)?;
                Self::Rgb { r, g, b }
            }
            ColorType::Indexed => {
                let index = u8::parse(reader, opts)?;
                Self::PaletteIndex { index }
            }
            ColorType::Unknown(o) => panic!("unknown color type: {o:?}"),
        })
    }

    pub async fn parse_from_color_type_async<
        O: atom_parser::Offset + Unpin,
        R: atom_parser::AsyncReader<O> + Unpin,
    >(
        ty: &ColorType,
        reader: &mut R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Self, atom_parser::ParseError> {
        use atom_parser::AsyncParse;
        Ok(match ty {
            ColorType::Greyscale | ColorType::GreyscaleWithAlpha => {
                let greyscale = u16::parse_async(reader, opts).await?;
                Self::Greyscale { greyscale }
            }
            ColorType::Rgb | ColorType::Rgba => {
                let [r, g, b] = <[u16; 3]>::parse_async(reader, opts).await?;
                Self::Rgb { r, g, b }
            }
            ColorType::Indexed => {
                let index = u8::parse_async(reader, opts).await?;
                Self::PaletteIndex { index }
            }
            ColorType::Unknown(o) => panic!("unknown color type: {o:?}"),
        })
    }
}

impl<T: atom_parser::Parsed, O> core::fmt::Debug for SuggestedPaletteEntry<T, O>
where
    <T as atom_parser::Parsed>::Output<O>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SuggestedPaletteEntry")
            .field("red", &self.red)
            .field("green", &self.green)
            .field("blue", &self.blue)
            .field("alpha", &self.alpha)
            .field("frequency", &self.frequency)
            .finish()
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"tIME"))]
    struct TimestampInfo {
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    }
}

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"acTL"))]
    struct AnimationInfo {
        frame_count: u32,
        repeat: u32,
    }

    #[derive(Debug)]
    #[enum_repr(u8)]
    enum FrameDispose {
        None = 0,
        Background = 1,
        Previous = 2,
    }

    #[derive(Debug)]
    #[enum_repr(u8)]
    enum Blend {
        Source = 0,
        Over = 1,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"fcTL"))]
    struct FrameControl {
        sequence_num: u32,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
        delay_num: u16,
        delay_den: u16,
        dispose: FrameDispose,
        blend: Blend,
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"fdAT"))]
    struct FrameData {
        sequence_num: u32,
        #[payload]
        data: Vec<u8>,
    }
}

#[cfg(test)]
const TEST_FILE: &str = "../3rd_order_intermod_animation_(thumbnail).png";

#[test]
fn png() {
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
    use atom_parser::Reader;

    let file = PngFile::parse(&mut r, &opts).unwrap();

    let mut header = None;

    let mut iccp = None;
    let mut srgb = None;
    let mut sbit = None;
    let mut gamma = None;
    let mut colorspace = None;
    let mut stretch = None;
    let mut palette = None;
    let mut suggested_palette = None;

    let mut mdcv = None;
    let mut clli = None;
    let mut hdr_reference = None;

    let mut histogram = None;
    let mut background = None;
    let mut transparency = None;

    let mut anim_header = None;
    let mut frame_control = None;

    let mut frame_data = vec![];

    let mut chunk_iter = file.chunks(&mut r, &opts).unwrap();
    while let Some(chunk) = chunk_iter.next() {
        let chunk = chunk.unwrap();
        use chunk::Chunk;
        let r = chunk_iter.reader();
        match chunk.chunk {
            Chunk::ImageHeader(hdr) if header.is_none() => header = Some(hdr),
            Chunk::IccProfile(profile) if iccp.is_none() => iccp = Some(profile),
            Chunk::SrgbColorSpace(cs) if srgb.is_none() => srgb = Some(cs),
            Chunk::SignificantBits(sb) if sbit.is_none() => {
                let color_type = &header.as_ref().expect("no image header").color_type;
                let mut r = atom_parser::BacktrackReader::from_extent(r, sb.bytes.extent());
                let bits =
                    SignificantBitsInfo::parse_from_color_type(color_type, &mut r, &opts).unwrap();
                sbit = Some(bits);
            }
            Chunk::SuggestedPalette(suggested) if suggested_palette.is_none() => {
                suggested_palette = Some(suggested)
            }
            Chunk::Gamma(g) if gamma.is_none() => gamma = Some(g),
            Chunk::ColorSpaceInfo(c) if colorspace.is_none() => colorspace = Some(c),
            Chunk::PhysicalPixelDimensions(s) if stretch.is_none() => stretch = Some(s),
            Chunk::Palette(p) if palette.is_none() => palette = Some(p),
            Chunk::ImageHistogram(h) if histogram.is_none() => histogram = Some(h),
            Chunk::BackgroundColor(bg) if background.is_none() => {
                let color_type = &header.as_ref().expect("no image header").color_type;
                let mut r = atom_parser::BacktrackReader::from_extent(r, bg.bytes.extent());
                let bg =
                    BackgroundColorInfo::parse_from_color_type(color_type, &mut r, &opts).unwrap();
                background = Some(bg);
            }
            Chunk::Transparency(t) if transparency.is_none() => {
                let color_type = &header.as_ref().expect("no image header").color_type;
                let mut r = atom_parser::BacktrackReader::from_extent(r, t.bytes.extent());
                let trns =
                    TransparencyInfo::parse_from_color_type(color_type, &mut r, &opts).unwrap();
                transparency = Some(trns);
            }
            Chunk::DisplayColorVolume(m) if mdcv.is_none() => mdcv = Some(m),
            Chunk::ContentLightLevel(li) if clli.is_none() => clli = Some(li),
            Chunk::HDRReferenceWhiteLevel(hrwl) if hdr_reference.is_none() => {
                hdr_reference = Some(hrwl)
            }

            Chunk::ImageData(dat) => {
                let data = r.read_bytes(dat.data.extent()).unwrap();
                frame_data.extend_from_slice(&data);
            }

            Chunk::AnimationInfo(actl) if anim_header.is_none() => anim_header = Some(actl),
            Chunk::FrameControl(fctl) => {
                assert!(anim_header.is_some());
                let frame = core::mem::take(&mut frame_data);
                let mut fctl = Some(fctl);
                core::mem::swap(&mut fctl, &mut frame_control);
                if let Some(prev_ctrl) = fctl {
                    // TODO: decode png data
                    println!(
                        "animated frame data: {frame:?}, {prev_ctrl:?}, {:?}",
                        anim_header.as_ref().unwrap()
                    );
                }
            }
            Chunk::FrameData(fdat) => {
                let data = r.read_bytes(fdat.data.extent()).unwrap();
                frame_data.extend_from_slice(&data);
            }

            Chunk::ImageEnd(_) => {
                assert_eq!(r.remaining_size(), 0);
                let frame = core::mem::take(&mut frame_data);
                match (anim_header.as_ref(), core::mem::take(&mut frame_control)) {
                    (Some(header), Some(ctrl)) => {
                        // TODO: decode png data
                        println!("last animated frame data: {frame:?}, {header:?}, {ctrl:?}");
                    }
                    (None, None) => {
                        // TODO: decode png data
                        println!("frame data: {frame:?}");
                    }
                    _ => panic!("invalid state"),
                }
            }

            Chunk::TimestampInfo(ts) => println!("timestamp: {ts:?}"),
            Chunk::Text(txt) => {
                let keyword = txt.keyword.extent();
                assert!(keyword.len < 80);
                let mut buf = [0; 128];
                r.read_extent(keyword, &mut buf).unwrap();
                let keyword = String::from_utf8_lossy(&buf[..keyword.len as usize]);
                let text = r.read_bytes(txt.text.extent()).unwrap();
                let text = String::from_utf8_lossy(&text);
                println!("text: {keyword} = {text}");
            }
            Chunk::CompressedText(ctxt) => {
                let keyword = ctxt.keyword.extent();
                assert!(keyword.len < 80);
                let mut buf = [0; 128];
                r.read_extent(keyword, &mut buf).unwrap();
                let keyword = String::from_utf8_lossy(&buf[..keyword.len as usize]);
                let compressed_text = r.read_bytes(ctxt.compressed_text.extent()).unwrap();
                println!("text: {keyword} = {compressed_text:?} (compressed)");
            }
            Chunk::InternationalText(itxt) => {
                let keyword = itxt.keyword.extent();
                assert!(keyword.len < 80);
                let mut buf = [0; 128];
                r.read_extent(keyword, &mut buf).unwrap();
                let keyword = String::from_utf8_lossy(&buf[..keyword.len as usize]);

                let translated_keyword = itxt.translated_keyword.extent();
                assert!(translated_keyword.len < 80);
                let mut buf = [0; 128];
                r.read_extent(translated_keyword, &mut buf).unwrap();
                let translated_keyword =
                    String::from_utf8_lossy(&buf[..translated_keyword.len as usize]);

                let text = r.read_bytes(itxt.text.extent()).unwrap();
                let text = String::from_utf8_lossy(&text);
                println!(
                    "text: {keyword} (translated {translated_keyword}) = {text} (compressed = {})",
                    itxt.compressed
                );
            }
            _ => (),
        }
    }
    panic!()
}

#[tokio::test]
async fn png_async() {
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
    use atom_parser::{AsyncReader, PollReader};

    let file = PngFile::parse_async(&mut r, &opts).await.unwrap();

    let mut header = None;

    let mut iccp = None;
    let mut srgb = None;
    let mut sbit = None;
    let mut gamma = None;
    let mut colorspace = None;
    let mut stretch = None;
    let mut palette = None;
    let mut suggested_palette = None;

    let mut mdcv = None;
    let mut clli = None;
    let mut hdr_reference = None;

    let mut histogram = None;
    let mut background = None;
    let mut transparency = None;

    let mut anim_header = None;
    let mut frame_control = None;

    let mut frame_data = vec![];

    use tokio_stream::StreamExt;
    let mut chunk_iter = file.chunks_async(&mut r, &opts).unwrap();
    while let Some(chunk) = chunk_iter.next().await {
        let chunk = chunk.unwrap();
        use chunk::Chunk;
        let r = chunk_iter.reader();
        match chunk.chunk {
            Chunk::ImageHeader(hdr) if header.is_none() => header = Some(hdr),
            Chunk::IccProfile(profile) if iccp.is_none() => iccp = Some(profile),
            Chunk::SrgbColorSpace(cs) if srgb.is_none() => srgb = Some(cs),
            Chunk::SignificantBits(sb) if sbit.is_none() => {
                let color_type = &header.as_ref().expect("no image header").color_type;
                let mut r = atom_parser::BacktrackReader::from_extent(r, sb.bytes.extent());
                let bits =
                    SignificantBitsInfo::parse_from_color_type_async(color_type, &mut r, &opts)
                        .await
                        .unwrap();
                sbit = Some(bits);
            }
            Chunk::SuggestedPalette(suggested) if suggested_palette.is_none() => {
                suggested_palette = Some(suggested)
            }
            Chunk::Gamma(g) if gamma.is_none() => gamma = Some(g),
            Chunk::ColorSpaceInfo(c) if colorspace.is_none() => colorspace = Some(c),
            Chunk::PhysicalPixelDimensions(s) if stretch.is_none() => stretch = Some(s),
            Chunk::Palette(p) if palette.is_none() => palette = Some(p),
            Chunk::ImageHistogram(h) if histogram.is_none() => histogram = Some(h),
            Chunk::BackgroundColor(bg) if background.is_none() => {
                let color_type = &header.as_ref().expect("no image header").color_type;
                let mut r = atom_parser::BacktrackReader::from_extent(r, bg.bytes.extent());
                let bg =
                    BackgroundColorInfo::parse_from_color_type_async(color_type, &mut r, &opts)
                        .await
                        .unwrap();
                background = Some(bg);
            }
            Chunk::Transparency(t) if transparency.is_none() => {
                let color_type = &header.as_ref().expect("no image header").color_type;
                let mut r = atom_parser::BacktrackReader::from_extent(r, t.bytes.extent());
                let trns = TransparencyInfo::parse_from_color_type_async(color_type, &mut r, &opts)
                    .await
                    .unwrap();
                transparency = Some(trns);
            }
            Chunk::DisplayColorVolume(m) if mdcv.is_none() => mdcv = Some(m),
            Chunk::ContentLightLevel(li) if clli.is_none() => clli = Some(li),
            Chunk::HDRReferenceWhiteLevel(hrwl) if hdr_reference.is_none() => {
                hdr_reference = Some(hrwl)
            }

            Chunk::ImageData(dat) => {
                let data = r.async_read_bytes(dat.data.extent()).await.unwrap();
                frame_data.extend_from_slice(&data);
            }

            Chunk::AnimationInfo(actl) if anim_header.is_none() => anim_header = Some(actl),
            Chunk::FrameControl(fctl) => {
                assert!(anim_header.is_some());
                let frame = core::mem::take(&mut frame_data);
                let mut fctl = Some(fctl);
                core::mem::swap(&mut fctl, &mut frame_control);
                if let Some(prev_ctrl) = fctl {
                    // TODO: decode png data
                    println!(
                        "animated frame data: {frame:?}, {prev_ctrl:?}, {:?}",
                        anim_header.as_ref().unwrap()
                    );
                }
            }
            Chunk::FrameData(fdat) => {
                let data = r.async_read_bytes(fdat.data.extent()).await.unwrap();
                frame_data.extend_from_slice(&data);
            }

            Chunk::ImageEnd(_) => {
                assert_eq!(r.remaining_size(), 0);
                let frame = core::mem::take(&mut frame_data);
                match (anim_header.as_ref(), core::mem::take(&mut frame_control)) {
                    (Some(header), Some(ctrl)) => {
                        // TODO: decode png data
                        println!("last animated frame data: {frame:?}, {header:?}, {ctrl:?}");
                    }
                    (None, None) => {
                        // TODO: decode png data
                        println!("frame data: {frame:?}");
                    }
                    _ => panic!("invalid state"),
                }
            }

            Chunk::TimestampInfo(ts) => println!("timestamp: {ts:?}"),
            Chunk::Text(txt) => {
                let keyword = txt.keyword.extent();
                assert!(keyword.len < 80);
                let mut buf = [0; 128];
                r.async_read_extent(keyword, &mut buf).await.unwrap();
                let keyword = String::from_utf8_lossy(&buf[..keyword.len as usize]);
                let text = r.async_read_bytes(txt.text.extent()).await.unwrap();
                let text = String::from_utf8_lossy(&text);
                println!("text: {keyword} = {text}");
            }
            Chunk::CompressedText(ctxt) => {
                let keyword = ctxt.keyword.extent();
                assert!(keyword.len < 80);
                let mut buf = [0; 128];
                r.async_read_extent(keyword, &mut buf).await.unwrap();
                let keyword = String::from_utf8_lossy(&buf[..keyword.len as usize]);
                let compressed_text = r
                    .async_read_bytes(ctxt.compressed_text.extent())
                    .await
                    .unwrap();
                println!("text: {keyword} = {compressed_text:?} (compressed)");
            }
            Chunk::InternationalText(itxt) => {
                let keyword = itxt.keyword.extent();
                assert!(keyword.len < 80);
                let mut buf = [0; 128];
                r.async_read_extent(keyword, &mut buf).await.unwrap();
                let keyword = String::from_utf8_lossy(&buf[..keyword.len as usize]);

                let translated_keyword = itxt.translated_keyword.extent();
                assert!(translated_keyword.len < 80);
                let mut buf = [0; 128];
                r.async_read_extent(translated_keyword, &mut buf)
                    .await
                    .unwrap();
                let translated_keyword =
                    String::from_utf8_lossy(&buf[..translated_keyword.len as usize]);

                let text = r.async_read_bytes(itxt.text.extent()).await.unwrap();
                let text = String::from_utf8_lossy(&text);
                println!(
                    "text: {keyword} (translated {translated_keyword}) = {text} (compressed = {})",
                    itxt.compressed
                );
            }
            _ => (),
        }
    }
    panic!()
}
