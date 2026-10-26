use atom_parser::make_atom;

make_atom! {
    #[derive(Debug)]
    #[enum_repr([u8; 6])]
    enum FormatVersion {
        Gif87 = *b"GIF87a",
        Gif89 = *b"GIF89a",
    }

    #[bitfield_repr(u8)]
    struct LogicalScreenDescriptor {
        #[bits(3)]
        color_table_size: u8,
        #[flag]
        color_table_sorted: bool,
        #[bits(3)]
        color_resolution: u8,
        #[flag]
        global_color_table: bool,
    }

    #[derive(Debug)]
    struct Header {
        version: FormatVersion,
        width: u16,
        height: u16,
        packed: LogicalScreenDescriptor,
        bg_color_index: u8,
        aspect_ratio: u8,
        #[payload(u16, packed.color_table_entry_count() * 3)]
        global_color_table: Vec<u8>,
    }
}

impl LogicalScreenDescriptor {
    pub fn color_table_entry_count(&self) -> u16 {
        if self.global_color_table() {
            1 << ((self.color_table_size() as u16) + 1)
        } else {
            0
        }
    }
}

impl Header {
    pub fn aspect_ratio(&self) -> f64 {
        (self.aspect_ratio as f64 + 15.) / 64.
    }
}
make_atom! {
    #[derive(Debug)]
    #[enum_repr(u8)]
    enum SegmentKind {
        Extension = 0x21,
        ImageBlock = 0x2c,
        Trailer = 0x3b,
    }

    #[derive(Debug)]
    struct Segment {
        #[condition]
        kind: SegmentKind,
        #[conditional(Segment)]
        segment: {
            match kind {
                SegmentKind::Extension => {
                    Extension {
                        extension: ExtensionBlock,
                    }
                }
                SegmentKind::ImageBlock => {
                    Image {
                        image: ImageBlock,
                    }
                }
                SegmentKind::Trailer => {
                    Trailer {
                        trailer: TrailerBlock,
                    }
                }
                SegmentKind::Unknown(_) => {
                    Unknown {
                        #[reuse_condition]
                        kind: SegmentKind,
                    }
                }
            }
        }
    }

    #[derive(Debug)]
    #[enum_repr(u8)]
    enum ExtensionKind {
        GraphicsControl = 0xf9,
        Plaintext = 0x01,
        Application = 0xff,
        Comment = 0xfe,
    }

    #[bitfield_repr(u8)]
    struct GraphicsControlPacked {
        #[flag]
        transparent_color: bool,
        #[flag]
        user_input: bool,
        #[enum_bits(3)]
        disposal_method: enum DisposalMethod {
            Unspecified = 0x00,
            DoNotDispose = 0x01,
            OverwriteWithBackground = 0x02,
            OverwriteWithPreviousGraphic = 0x04,
        },
        #[reserved]
        #[bits(3)]
        reserved: u8,
    }

    #[derive(Debug)]
    struct ExtensionBlock {
        #[condition]
        kind: ExtensionKind,
        #[conditional(Extension)]
        extension: {
            match kind {
                ExtensionKind::GraphicsControl => {
                    GraphicsControl {
                        #[expected(0x04)]
                        block_size: u8,
                        packed: GraphicsControlPacked,
                        delay_cs: u16,
                        transparent_color_idx: u8,
                        #[expected(0x00)]
                        terminator: u8,
                    }
                }
                ExtensionKind::Plaintext => {
                    Plaintext {
                        #[expected(0x0c)]
                        block_size: u8,
                        text_left: u16,
                        text_top: u16,
                        text_width: u16,
                        text_height: u16,
                        cell_width: u16,
                        cell_height: u16,
                        text_fg_idx: u8,
                        text_bg_idx: u8,
                        data: SubBlocks,
                    }
                }
                ExtensionKind::Application => {
                    Application {
                        #[expected(0x0b)]
                        block_size: u8,
                        identifier: [u8; 8],
                        authentication_code: [u8; 3],
                        data: SubBlocks,
                    }
                }
                ExtensionKind::Comment => {
                    Comment {
                        data: SubBlocks,
                    }
                }
                ExtensionKind::Unknown(_) => {
                    Unknown {
                        #[reuse_condition]
                        kind: ExtensionKind,
                    }
                }
            }
        }
    }

    #[bitfield_repr(u8)]
    struct LocalImageDescriptor {
        #[flag]
        local_color_table: bool,
        #[flag]
        interlace: bool,
        #[flag]
        color_table_sort: bool,
        #[reserved]
        #[bits(2)]
        reserved: u8,
        #[bits(3)]
        color_table_size: u8,
    }

    #[derive(Debug)]
    struct ImageHeader {
        left: u16,
        top: u16,
        width: u16,
        height: u16,
        packed: LocalImageDescriptor,
        #[payload(u16, packed.color_table_entry_count() * 3)]
        color_table: Vec<u8>,
    }

    #[derive(Debug)]
    struct ImageBlock {
        header: ImageHeader,
        lzw_min_code_size: u8,
        data: SubBlocks,
    }

    #[derive(Debug)]
    struct TrailerBlock {

    }
}

impl LocalImageDescriptor {
    pub fn color_table_entry_count(&self) -> u16 {
        if self.local_color_table() {
            1 << ((self.color_table_size() as u16) + 1)
        } else {
            0
        }
    }
}

impl<O> extension_block::GraphicsControl<O> {
    pub fn transparent_color_index(&self) -> Option<u8> {
        if self.packed.transparent_color() {
            Some(self.transparent_color_idx)
        } else {
            None
        }
    }
}

make_atom! {
    #[derive(Debug)]
    struct SubBlock {
        #[payload_length]
        len: u8,
        #[payload]
        data: Vec<u8>,
    }

    #[derive(Debug)]
    struct SubBlocks {
        #[null_terminated_array]
        sub_blocks: SubBlock,
    }
}

impl<O: atom_parser::Offset> SubBlocks<O> {
    pub fn read_into<R: atom_parser::Reader<O>>(
        &self,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Box<[u8]>, atom_parser::ParseError> {
        let mut subblock_iter = self.sub_blocks(&mut reader, opts);
        let mut subblock_buf = [0u8; 255];
        let mut buf = vec![];

        while let Some(subblock) = subblock_iter.next() {
            let reader = subblock_iter.reader();
            let subblock = subblock?;
            let extent = subblock.data.extent();
            use atom_parser::Reader;
            reader.read_extent(extent, &mut subblock_buf)?;
            buf.extend_from_slice(&subblock_buf[..(extent.len as usize)]);
        }
        Ok(buf.into_boxed_slice())
    }

    pub async fn read_into_async<
        R: atom_parser::PollReader<O> + Unpin + atom_parser::SwapOffsets<O>,
    >(
        &self,
        mut reader: R,
        opts: &atom_parser::ParseOptions,
    ) -> Result<Box<[u8]>, atom_parser::ParseError>
    where
        O: Unpin,
    {
        let mut subblock_iter = self.sub_blocks_async(&mut reader, opts);
        let mut subblock_buf = [0u8; 255];
        let mut buf = vec![];

        use tokio_stream::StreamExt;
        while let Some(subblock) = subblock_iter.next().await {
            let reader = subblock_iter.reader();
            let subblock = subblock?;
            let extent = subblock.data.extent();
            use atom_parser::AsyncReader;
            reader.async_read_extent(extent, &mut subblock_buf).await?;
            buf.extend_from_slice(&subblock_buf[..(extent.len as usize)]);
        }
        Ok(buf.into_boxed_slice())
    }
}

impl<O: atom_parser::Offset> core::cmp::PartialEq for SubBlock<O> {
    fn eq(&self, other: &Self) -> bool {
        self.data.extent().len == other.data.extent().len
    }
}

make_atom! {
    #[derive(Debug)]
    struct Gif {
        header: Header,
        #[trailing_array]
        segments: Segment,
    }
}

#[cfg(test)]
const TEST_FILE: &str = "../test_gif.gif";

#[test]
fn gif() {
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

    let gif = Gif::parse(&mut r, &opts).unwrap();
    println!("gif: {gif:?}");
    let mut segment_iter = gif.segments(&mut r, &opts).unwrap();

    let mut graphics_control = None;
    while let Some(segment) = segment_iter.next() {
        use segment::Segment;
        let r = segment_iter.reader();
        match segment.unwrap().segment {
            Segment::Extension(segment::Extension { extension, .. }) => {
                use extension_block::Extension;
                match extension.extension {
                    Extension::GraphicsControl(ctrl) => {
                        println!("graphics control: {ctrl:?}");
                        graphics_control = Some(ctrl);
                    }
                    Extension::Plaintext(text) => {
                        let text_data = text.data.read_into(r, &opts).unwrap();
                        let text_str = String::from_utf8_lossy(&text_data);
                        println!("plaintext: {text:?}, data: {text_str:?}");
                    }
                    Extension::Application(app) => {
                        let app_data = app.data.read_into(r, &opts).unwrap();
                        println!(
                            "app: {app:?}, app data: {app_data:?} (len: {})",
                            app_data.len()
                        );
                    }
                    Extension::Comment(comment) => {
                        let comment = comment.data.read_into(r, &opts).unwrap();
                        let str = String::from_utf8_lossy(&comment);
                        println!("comment: {str:?}");
                    }
                    Extension::Unknown(_) => panic!("unknown extension"),
                }
            }
            Segment::Image(img) => {
                let image_data = img.image.data.read_into(r, &opts).unwrap();
                println!(
                    "image: {img:?}, data: {image_data:?} (len: {})",
                    image_data.len()
                );
                if let Some(_ctrl) = core::mem::take(&mut graphics_control) {}
                // TODO: LZW decoding
            }
            Segment::Trailer(_) => {
                use atom_parser::Reader;
                if segment_iter.reader().remaining_size() > 0 {
                    panic!("more data after trailer")
                }
            }
            Segment::Unknown(_) => panic!(),
        }
    }

    panic!()
}

#[tokio::test]
async fn gif_async() {
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

    let gif = Gif::parse_async(&mut r, &opts).await.unwrap();
    println!("gif: {gif:?}");
    let mut segment_iter = gif.segments_async(&mut r, &opts).unwrap();

    use tokio_stream::StreamExt;
    let mut graphics_control = None;
    while let Some(segment) = segment_iter.next().await {
        use segment::Segment;
        let r = segment_iter.reader();

        match segment.unwrap().segment {
            Segment::Extension(segment::Extension { extension, .. }) => {
                use extension_block::Extension;
                match extension.extension {
                    Extension::GraphicsControl(ctrl) => {
                        println!("graphics control: {ctrl:?}");
                        graphics_control = Some(ctrl);
                    }
                    Extension::Plaintext(text) => {
                        let text_data = text.data.read_into_async(r, &opts).await.unwrap();
                        let text_str = String::from_utf8_lossy(&text_data);
                        println!("plaintext: {text:?}, data: {text_str:?}");
                    }
                    Extension::Application(app) => {
                        let app_data = app.data.read_into_async(r, &opts).await.unwrap();
                        println!(
                            "app: {app:?}, app data: {app_data:?} (len: {})",
                            app_data.len()
                        );
                    }
                    Extension::Comment(comment) => {
                        let comment = comment.data.read_into_async(r, &opts).await.unwrap();
                        let str = String::from_utf8_lossy(&comment);
                        println!("comment: {str:?}");
                    }
                    Extension::Unknown(_) => panic!("unknown extension"),
                }
            }
            Segment::Image(img) => {
                let image_data = img.image.data.read_into_async(r, &opts).await.unwrap();
                println!(
                    "image: {img:?}, data: {image_data:?} (len: {})",
                    image_data.len()
                );
                if let Some(_ctrl) = core::mem::take(&mut graphics_control) {}
                // TODO: LZW decoding
            }
            Segment::Trailer(_) => {
                use atom_parser::PollReader;
                if segment_iter.reader().remaining_size() > 0 {
                    panic!("more data after trailer")
                }
            }
            Segment::Unknown(_) => panic!(),
        }
    }

    panic!()
}
