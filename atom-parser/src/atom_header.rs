use crate::{
    AsyncParse, AtomSize, AtomSizeParse, FourCC, Parse, ParseError, ParseOptions, PollReader,
    Reader, TakeReader, impl_take_reader, Offset
};

#[derive(Debug)]
pub struct AtomHeader {
    pub size: AtomSize,
    pub fcc: FourCC,
}

impl <O: Offset> Parse<O> for AtomHeader {
    fn parse<T: Reader<O>>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = u32::parse(reader, options)?;
        let fcc = FourCC::parse(reader, options)?;
        let size = AtomSize::parse(size, reader, options)?;
        Ok(Self { size, fcc })
    }
}

pub enum AtomHeaderParse<'a, O: Offset + Unpin, R: PollReader<O> + Unpin> {
    Waiting(<u32 as AsyncParse<O>>::Fut<'a, R>),
    Fourcc {
        size: u32,
        fourcc: <FourCC as AsyncParse<O>>::Fut<'a, R>,
    },
    AtomSize {
        fourcc: FourCC,
        // the u32 size from before is moved here
        size: AtomSizeParse<'a, O, R>,
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin> Future for AtomHeaderParse<'a, O, R> {
    type Output = Result<AtomHeader, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Waiting(mut fut) => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Waiting(fut);
                        return Poll::Pending;
                    }
                    Poll::Ready(size) => {
                        let (reader, opts) = fut.take_reader();
                        let size = match size {
                            Ok(s) => s,
                            Err(e) => {
                                *self = Self::Done(reader, opts);
                                return Poll::Ready(Err(e));
                            }
                        };
                        *self = AtomHeaderParse::Fourcc {
                            size,
                            fourcc: FourCC::create_fut(reader, opts),
                        }
                    }
                },
                Self::Fourcc { size, mut fourcc } => match Pin::new(&mut fourcc).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Fourcc { size, fourcc };
                        return Poll::Pending;
                    }
                    Poll::Ready(fcc) => {
                        let (reader, opts) = fourcc.take_reader();
                        let fcc = match fcc {
                            Ok(fcc) => fcc,
                            Err(e) => {
                                *self = Self::Done(reader, opts);
                                return Poll::Ready(Err(e));
                            }
                        };

                        *self = Self::AtomSize {
                            fourcc: fcc,
                            size: AtomSize::create_fut(size, reader, opts),
                        };
                    }
                },
                Self::AtomSize { fourcc, mut size } => match Pin::new(&mut size).poll(cx) {
                    Poll::Pending => {
                        *self = Self::AtomSize { fourcc, size };
                        return Poll::Pending;
                    }
                    Poll::Ready(atom_size) => {
                        let (reader, opts) = size.take_reader();
                        *self = Self::Done(reader, opts);
                        return Poll::Ready(atom_size.map(|size| AtomHeader { size, fcc: fourcc }));
                    }
                },
                Self::Done(..) => panic!("repoll future"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin> TakeReader<'a, R> for AtomHeaderParse<'a, O, R> {
    impl_take_reader! {}

    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Waiting(r) => r.borrow_reader(),
            Self::Fourcc { fourcc, .. } => fourcc.borrow_reader(),
            Self::AtomSize { size, .. } => size.borrow_reader(),
            Self::Done(r, opts) => (r, opts),
            _ => unreachable!(),
        }
    }
}

impl <O: Offset + Unpin> AsyncParse<O> for AtomHeader {
    type Fut<'a, R: PollReader<O> + Unpin> = AtomHeaderParse<'a, O, R>;
    fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R> {
        AtomHeaderParse::Waiting(u32::create_fut(reader, options))
    }
}
