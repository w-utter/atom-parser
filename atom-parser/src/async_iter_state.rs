use crate::{ParseOptions, impl_take_reader, TakeReader};

pub enum AsyncIterState<'a, R, T> {
    Iterating(T),
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R, T: TakeReader<'a, R>> TakeReader<'a, R> for AsyncIterState<'a, R, T> {
    impl_take_reader!{}
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Iterating(t) => t.borrow_reader(),
            Self::Done(r, opts) => (r, opts),
            Self::Empty => unreachable!(),
        }
    }
}

