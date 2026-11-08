use crate::{TakeReader, impl_take_reader};

pub enum AsyncIterState<P, R, T> {
    Iterating(T),
    Done(R, P),
    Empty,
}

impl<P, R, T: TakeReader<P, R>> TakeReader<P, R> for AsyncIterState<P, R, T> {
    impl_take_reader! {}
    fn borrow_reader(&mut self) -> (&mut R, &P) {
        match self {
            Self::Iterating(t) => t.borrow_reader(),
            Self::Done(r, opts) => (r, opts),
            Self::Empty => unreachable!(),
        }
    }
}
