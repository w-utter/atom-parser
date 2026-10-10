use atom_parser::make_atom;

make_atom! {
    #[derive(Debug)]
    #[enum_repr(u32)]
    enum MagicNumber {
        UsingMicroseconds = 0xA1B2C3D4,
        UsingNanoseconds = 0xA1B23C4D,
    }

    #[derive(Debug)]
    struct Header {
        magic_num: MagicNumber,
        major_version: u16,
        minor_version: u16,
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
        header: Header,
        #[trailing_array]
        records: Record,
    }
}

#[cfg(test)]
const TEST_FILE: &str = "../telnet-raw.pcap";

#[test]
fn pcap() {
    let mut r = atom_parser::InMemoryReader::from_path(TEST_FILE).unwrap();
    let opts = atom_parser::ParseOptions {
        endianess: atom_parser::Endianess::Little,
        ..Default::default()
    };
    use atom_parser::Parse;

    let file = PcapFile::parse(&mut r, &opts).unwrap();
    println!("file: {file:?}");

    let records = file
        .records(&mut r, &opts)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    println!("records: {records:?} (len: {})", records.len());
    panic!()
}

#[tokio::test]
async fn pcap_async() {
    let mut r = atom_parser::InMemoryReader::from_path(TEST_FILE).unwrap();
    let opts = atom_parser::ParseOptions {
        endianess: atom_parser::Endianess::Little,
        ..Default::default()
    };
    use atom_parser::AsyncParse;
    let file = PcapFile::parse_async(&mut r, &opts).await.unwrap();
    println!("file: {file:?}");

    use tokio_stream::StreamExt;
    let records = file
        .records_async(&mut r, &opts)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .await
        .unwrap();
    println!("records: {records:?} (len: {})", records.len());
    panic!()
}
