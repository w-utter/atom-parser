use atom_parser::make_atom;

/*
make_atom! {
    #[enum_repr(u32)]
    enum MagicNumber {
        UsingMicroseconds = 0xA1B2C3D4,
        UsingNanoseconds = 0xA1B23C4D,
    }

    struct Header {
        magic_num: MagicNumber,
        major_version: u16,
        minor_version: u16,
        reserved: [u8; 8],
        snap_len: u32,
        fcs: u16,
        link_type: u16,
    }

    struct Record {
        second_ts: u32,
        subsecond_ts: u32,
        #[payload_length]
        captured_packet_len: u32,
        original_packet_len: u32,
        #[payload]
        packet: Vec<u8>,
    }
}

#[test]
fn pcap() {
    let mut r = atom_parser::InMemoryReader::from_path("../telnet-raw.pcap").unwrap();
    let opts = atom_parser::ParseOptions {
        endianess: atom_parser::Endianess::Little,
        ..Default::default()
    };
    use atom_parser::Parse;
    let header = Header::parse(&mut r, &opts).unwrap();

    use atom_parser::Reader;
    println!("header: {header:?}");
    while r.remaining_size() > 0 {
        let record = Record::parse(&mut r, &opts).unwrap();
        println!("packet: {record:?}");
        r.seek(record.packet.extent().len as usize).unwrap();
    }
    panic!()
}

#[tokio::test]
async fn pcap_async() {
    let mut r = atom_parser::InMemoryReader::from_path("../telnet-raw.pcap").unwrap();
    let opts = atom_parser::ParseOptions {
        endianess: atom_parser::Endianess::Little,
        ..Default::default()
    };
    use atom_parser::AsyncParse;
    let header = Header::parse_async(&mut r, &opts).await.unwrap();

    use atom_parser::Reader;
    println!("header: {header:?}");
    while r.remaining_size() > 0 {
        let record = Record::parse_async(&mut r, &opts).await.unwrap();
        println!("packet: {record:?}");
        atom_parser::AsyncSeek::seek(&mut r, record.packet.extent().len as usize)
            .await
            .unwrap();
    }
    panic!()
}
*/

trait Something {
    type Assoc;
}
struct A<T> {
    inner: T,
}

impl <T> Something for A<T> {
    type Assoc = T;
}

impl Something for u32 {
    type Assoc = Self;
}

// TODO: the proc macro needs to take in a generic O now
// - its probably best to have a phantomdata <O> for all collections of data
//      - may need to have an associated type Output<O> = Self
//          - issue is that the offset is only applicable to some fields
//          - e.g, u16/u32 dont need a <O> but any collection of data does
//  
// - or, scrap this and replace `usize` with `Offset` which can be configured at compile time.
//      - e.g, 
//          #[cfg(feature = "offset_width_usize")]
//          type Offset = usize;
//          #[cfg(feature = "offset_width_u64")]
//          type Offset = u64;
//          #[cfg(feature = "offset_width_u32")]
//          type Offset = u32;
//      for all pre existing instances of usize
//
//  - orrrr
//      - move the offset size to the end of the generics
//      e.g, SomtStruct<A, B, O: Offset = usize> {...}
//
//      then have a newtrait 
//      trait Parsed {
//          type Output<O>;
//      }
//
//      then impl <A, B> Parsed for SomStruct<A, B> {
//          type Output<O> = SomeStruct<A, B, O>;
//      }
//
//      then the full type can be referred to as 
//      <SomeStruct<A, B> as Parsed>::Output<O>;
//      which should allow it to be generic over the offset
