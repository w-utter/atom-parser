#[derive(Debug, PartialEq, Eq)]
pub struct UnknownVersion<O, V> {
    pub version: V,
    _offset: core::marker::PhantomData<O>,
}

impl<O, V> UnknownVersion<O, V> {
    pub fn new(version: V) -> Self {
        Self {
            version,
            _offset: core::marker::PhantomData,
        }
    }
}
