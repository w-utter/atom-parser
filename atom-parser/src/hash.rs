pub trait FullHasher: Default {
    type Hash;
    fn finish(self) -> Self::Hash;
    fn hash(&mut self, bytes: &[u8]);
}

pub trait Hasher {
    fn hash(&mut self, bytes: &[u8]);
}

impl<'a, H: FullHasher> Hasher for &'a mut H {
    fn hash(&mut self, bytes: &[u8]) {
        H::hash(self, bytes)
    }
}

// a hasher comprised of multiple hashers
// e.g, in case there is nested error detection
pub struct CombinedHasher<'a, H1, H2> {
    h1: &'a mut H1,
    h2: &'a mut H2,
}

impl<'a, H1: Hasher, H2: Hasher> CombinedHasher<'a, H1, H2> {
    pub fn combine_hashes(h1: &'a mut H1, h2: &'a mut H2) -> Self {
        Self { h1, h2 }
    }
}

impl<'a, H1: Hasher, H2: Hasher> Hasher for CombinedHasher<'a, H1, H2> {
    fn hash(&mut self, bytes: &[u8]) {
        self.h1.hash(bytes);
        self.h2.hash(bytes);
    }
}
