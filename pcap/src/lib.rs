use atom_parser::make_atom;

make_atom! {
    #[derive(Debug)]
    #[enum_repr(u32)]
    #[endianess(little)]
    enum MagicNumber {
        UsingMicrosecondsLE = 0xA1B2C3D4,
        UsingNanosecondsLE = 0x1A2B3C4D,
        UsingMicrosecondsBE = 0xD4C3B2A1,
        UsingNanosecondsBE = 0x4D3C2B1A,
    }

    #[derive(Debug)]
    struct Header {
        major_version: u16,
        minor_version: u16,
        #[reserved]
        reserved: [u8; 8],
        snap_len: u32,
        fcs: u16,
        link_type: u16,
    }

    #[derive(Debug)]
    struct Record {
        second_ts: u32,
        subsecond_ts: u32,
        #[payload_length]
        captured_packet_len: u32,
        original_packet_len: u32,
        #[payload]
        packet: Vec<u8>,
    }

    #[derive(Debug)]
    struct PcapFile {
        magic_num: MagicNumber,
        #[conditional(PcapEndianess)]
        file: {
            if magic_num.is_le() {
                Little {
                    #[endianess(little)]
                    pcap: Pcap,
                }
            } else {
                Big {
                    #[endianess(big)]
                    pcap: Pcap,
                }
            }
        }
    }

    #[derive(Debug)]
    struct Pcap {
        header: Header,
        #[trailing_array]
        records: Record,
    }
}

impl MagicNumber {
    pub fn is_le(&self) -> bool {
        matches!(self, Self::UsingMicrosecondsLE | Self::UsingNanosecondsLE)
    }

    pub fn is_be(&self) -> bool {
        matches!(self, Self::UsingMicrosecondsBE | Self::UsingNanosecondsBE)
    }

    pub fn using_nanoseconds(&self) -> bool {
        matches!(self, Self::UsingNanosecondsLE | Self::UsingNanosecondsBE)
    }

    pub fn using_microseconds(&self) -> bool {
        matches!(self, Self::UsingMicrosecondsLE | Self::UsingMicrosecondsBE)
    }
}

#[cfg(test)]
const TEST_FILE: &str = "../telnet-raw.pcap";

#[test]
fn pcap() {
    #[cfg(feature = "std")]
    let mut r = atom_parser::FileReader::<_, 1024>::from_path(TEST_FILE).unwrap();
    #[cfg(not(feature = "std"))]
    let mut r = {
        let bytes = std::fs::read(TEST_FILE).unwrap();
        atom_parser::InMemoryReader::from_buf(bytes)
    };
    let mut opts = atom_parser::ParseOptions::default();
    use atom_parser::Parse;

    let file = PcapFile::parse(&mut r, &opts).unwrap();
    println!("file: {file:?}");

    use pcap_file::PcapEndianess;
    let records = match file.file {
        PcapEndianess::Little(le) => {
            opts.endianess = atom_parser::Endianess::Little;
            le.pcap
                .records(&mut r, &opts)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        }
        PcapEndianess::Big(be) => {
            opts.endianess = atom_parser::Endianess::Big;
            be.pcap
                .records(&mut r, &opts)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        }
    };

    println!("records: {records:?} (len: {})", records.len());
    panic!()
}

#[tokio::test]
async fn pcap_async() {
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
    let mut opts = atom_parser::ParseOptions::default();
    use atom_parser::AsyncParse;
    let file = PcapFile::parse_async(&mut r, &opts).await.unwrap();
    println!("file: {file:?}");

    use tokio_stream::StreamExt;

    use pcap_file::PcapEndianess;
    let records = match file.file {
        PcapEndianess::Little(le) => {
            opts.endianess = atom_parser::Endianess::Little;
            le.pcap
                .records_async(&mut r, &opts)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .await
                .unwrap()
        }
        PcapEndianess::Big(be) => {
            opts.endianess = atom_parser::Endianess::Big;
            be.pcap
                .records_async(&mut r, &opts)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .await
                .unwrap()
        }
    };

    println!("records: {records:?} (len: {})", records.len());
    panic!()
}
