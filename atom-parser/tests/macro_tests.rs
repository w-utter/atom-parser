use atom_parser::make_atom;

make_atom! {
    struct A {
        f1: u8,
        f2: u16,
        f3: u32,
        f4: u64,
    }
}

make_atom! {
    struct B {
        #[dynamic_array(size_type = u16, zero_relative = false)]
        f: u64,
        #[pascal_string(u16)]
        pascal: String,
        #[null_terminated_string]
        null_terminated: String,
        #[reserved]
        reserved: [u8; 10],
        #[payload]
        payload: Vec<u8>,
    }

    struct B2 {
        #[dynamic_array(size_type = u16, zero_relative = false)]
        f: u64,
        #[pascal_string(u16)]
        pascal: String,
        #[null_terminated_string]
        null_terminated: String,
        #[reserved]
        reserved: [u8; 10],
        #[trailing_array]
        trailing: u32,
    }
}

make_atom! {
    #[version(0)]
    struct C {
        #[version]
        version: u16,
        b: u32,
    }
}

make_atom! {
    struct E {
        #[flags(u32)]
        flags: struct Flags {},
    }
}

make_atom! {
    #[version(0)]
    #[atom("efgh")]
    struct F {
        #[full_box]
        struct Flags { },
    }
}

make_atom! {
    #[atom("hijk")]
    struct G {
        #[children]
        enum Child { }
    }
}

make_atom! {
    #[atom("lmno")]
    struct H {
        #[children(u32)]
        enum Child {}
    }
}

make_atom! {
    struct I {
        #[payload_length]
        len: u32,
        #[payload]
        payload: Vec<u8>,
    }
}

make_atom! {
    struct J {
        #[payload_length]
        len: u32,
        #[trailing_array]
        payload: u32,
    }
}

#[test]
fn test_compile() {}

#[cfg(all(feature = "alloc", feature = "provided_readers"))]
#[test]
fn null_terminated() {
    make_atom! {
        struct A {
            #[null_terminated_array]
            null_terminated: u32,
            after: u8,
        }
    }

    let bytes = vec![
        0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 0, 128,
    ];
    let mut r = atom_parser::InMemoryReader::from_buf(bytes);
    let opts = Default::default();
    use atom_parser::Parse;
    let a = A::parse(&mut r, &opts).unwrap();
    let nums = a
        .null_terminated(&mut r, &opts)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(nums, vec![1, 2, 3, 4]);
    assert_eq!(a.after, 128);
}
