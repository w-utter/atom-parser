use crate::{AtomSize, FourCC, Parse, Reader, PollReader, AsyncParse, ParseOptions, TakeReader, ParseError, impl_take_reader, AtomSizeParse, IntegerParse};

#[derive(Debug)]
pub struct AtomHeader {
    pub size: AtomSize,
    pub fcc: FourCC,
}

impl Parse for AtomHeader {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = u32::parse(reader, options)?;
        let fcc = FourCC::parse(reader, options)?;
        let size = AtomSize::parse(size, reader, options)?;
        Ok(Self {
            size,
            fcc,
        })
    }
}

pub enum AtomHeaderParse<'a, R: PollReader + Unpin> {
    Waiting(<u32 as AsyncParse>::Fut<'a, R>),
    Fourcc {
        size: u32,
        fourcc: <FourCC as AsyncParse>::Fut<'a, R>,
    },
    AtomSize {
        fourcc: FourCC,
        // the u32 size from before is moved here
        size: AtomSizeParse<'a, R>,
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin> Future for AtomHeaderParse<'a, R> {
    type Output = Result<AtomHeader, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Waiting(mut fut) => {
                    match Pin::new(&mut fut).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Waiting(fut);
                            return Poll::Pending;
                        }
                        Poll::Ready(size) => {
                            let size = size?;
                            let IntegerParse::Done(reader, options) = fut else {
                                unreachable!("bad future state");
                            };
                            *self = AtomHeaderParse::Fourcc {
                                size,
                                fourcc: FourCC::create_fut(reader, options),
                            }
                        }
                    }
                }
                Self::Fourcc {
                    size,
                    mut fourcc,
                } => {
                    match Pin::new(&mut fourcc).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Fourcc { size, fourcc };
                            return Poll::Pending;
                        }
                        Poll::Ready(fcc) => {
                            let fcc = fcc?;
                            let IntegerParse::Done(reader, opts) = fourcc.inner else {
                                unreachable!("IntegerParse invalid state");
                            };
                            *self = Self::AtomSize{
                                fourcc: fcc, 
                                size: AtomSize::create_fut(size, reader, opts)
                            };
                        }
                    }
                }
                Self::AtomSize {
                    fourcc,
                    mut size,
                } => {
                    match Pin::new(&mut size).poll(cx) {
                        Poll::Pending => {
                            *self = Self::AtomSize { fourcc, size };
                            return Poll::Pending;
                        }
                        Poll::Ready(atom_size) => {
                            let AtomSizeParse::Done(reader, opts) = size else {
                                unreachable!("AtomSizeParse invalid state");
                            };
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(atom_size.map(|size| {
                                AtomHeader {
                                    size,
                                    fcc: fourcc,
                                }
                            }))
                        }
                    }
                }
                Self::Done(..) => panic!("repoll future"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for AtomHeaderParse<'a, R> {
    impl_take_reader!{}

    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Waiting(r) => r.borrow_reader(),
            Self::Fourcc { fourcc, .. } => fourcc.borrow_reader(),
            Self::AtomSize { size, ..} => size.borrow_reader(),
            Self::Done(r, opts) => (r, opts),
            _ => unreachable!(),
        }
    }
}

impl AsyncParse for AtomHeader {
    type Fut<'a, R: PollReader + Unpin> = AtomHeaderParse<'a, R>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        AtomHeaderParse::Waiting(u32::create_fut(reader, options))
    }
}

