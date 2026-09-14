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

#[test]
fn test_compile() {}
