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

pub struct WithBigEndian<T> {
    pub opts: T,
}

pub struct WithLittleEndian<T> {
    pub opts: T,
}

pub struct WithUserEndianess<T> {
    pub opts: T,
}

pub trait Options {
    fn options(&self) -> &ParseOptions;
    fn endianess(&self) -> &Endianess;
}

impl Options for ParseOptions {
    fn options(&self) -> &ParseOptions {
        self
    }
    fn endianess(&self) -> &Endianess {
        &self.endianess
    }
}

impl<T: Options> Options for WithBigEndian<T> {
    fn options(&self) -> &ParseOptions {
        self.opts.options()
    }
    fn endianess(&self) -> &Endianess {
        &Endianess::Big
    }
}

impl<T: Options> Options for WithUserEndianess<T> {
    fn options(&self) -> &ParseOptions {
        self.opts.options()
    }
    fn endianess(&self) -> &Endianess {
        &self.opts.options().endianess
    }
}

impl<T: Options> Options for WithLittleEndian<T> {
    fn options(&self) -> &ParseOptions {
        self.opts.options()
    }

    fn endianess(&self) -> &Endianess {
        &Endianess::Little
    }
}

impl<'a, T: Options> Options for &'a T {
    fn options(&self) -> &ParseOptions {
        T::options(self)
    }

    fn endianess(&self) -> &Endianess {
        T::endianess(self)
    }
}
