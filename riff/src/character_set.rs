use atom_parser::{FourCC, make_atom};

make_atom! {
    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"CSET"))]
    struct CharacterSet {
        code_page: u16,
        country_code: u16,
        language: u16,
        dialect: u16,
    }
}
