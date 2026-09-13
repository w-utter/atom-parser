#[derive(Debug, PartialEq, Eq, Default)]
pub enum Endianess {
    #[default]
    Big,
    Little,
}

#[derive(Default)]
pub struct ParseOptions {
    pub endianess: Endianess,
    pub error_on_used_reserved_fields: bool,
    pub error_on_missing_flags: bool,
    pub error_on_unknown_flags: bool,
}
