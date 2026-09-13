use crate::{Reader, PollReader, TakeReader, ParseError, ParseOptions, impl_take_reader};

#[cfg(feature = "extended_sized_atoms")]
use crate::{AsyncParse, IntegerParse, Parse};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AtomSize {
    #[cfg(not(feature = "extended_sized_atoms"))]
    size: u32,
    #[cfg(feature = "extended_sized_atoms")]
    size: u64,
}

impl AtomSize {
    #[cfg(feature = "extended_sized_atoms")]
    pub fn size(&self) -> Option<u64> {
        if self.until_eof() {
            None
        } else {
            Some(self.size)
        }
    }

    #[cfg(not(feature = "extended_sized_atoms"))]
    pub fn size(&self) -> Option<u32> {
        if self.until_eof() {
            None
        } else {
            Some(self.size)
        }
    }

    pub fn until_eof(&self) -> bool {
        #[cfg(feature = "extended_sized_atoms")]
        {
            self.size == u64::MAX
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            self.size == u32::MAX
        }
    }

    const MIN_ATOM_SIZE_32: u32 = 8;
    #[cfg(feature = "extended_sized_atoms")]
    const MIN_ATOM_SIZE_64: u64 = 16;

    pub fn parse<T: Reader>(size: u32, reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        #[cfg(feature = "extended_sized_atoms")]
        {
            if !matches!(size, 0 | 1) && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            let size = if size == 1 {
                let extended_size = u64::parse(reader, options)?;
                if extended_size < Self::MIN_ATOM_SIZE_64 {
                    return Err(ParseError::AtomSizeTooSmall)
                }
                extended_size - Self::MIN_ATOM_SIZE_64
            } else {
                if size == 0 {
                    u64::MAX
                } else {
                    (size - Self::MIN_ATOM_SIZE_32) as u64
                }
            };

            Ok(Self {
                size
            })
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            let _ = reader;
            let _ = options;
            if size == 1 {
                return Err(ParseError::AtomSizeUnsupported);
            }

            if size != 0 && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            let size = if size == 0 {
                u32::MAX
            } else {
                size - Self::MIN_ATOM_SIZE_32
            };

            Ok(Self {
                size,
            })
        }
    }

    pub fn create_fut<'a, R: PollReader + Unpin>(size: u32, reader: R, options: &'a ParseOptions) -> AtomSizeParse<'a, R> {
        #[cfg(feature = "extended_sized_atoms")]
        {
            if size == 1 {
                AtomSizeParse::ExtendedSize(u64::create_fut(reader, options))
            } else {
                AtomSizeParse::Waiting(size, reader, options)
            }
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            AtomSizeParse::Waiting(size, reader, options)
        }
    }
}

pub enum AtomSizeParse<'a, R: PollReader + Unpin> {
    Waiting(u32, R, &'a ParseOptions),
    #[cfg(feature = "extended_sized_atoms")]
    ExtendedSize(<u64 as AsyncParse>::Fut<'a, R>),
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin> Future for AtomSizeParse<'a, R> {
    type Output = Result<AtomSize, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        loop {
            let this = core::mem::replace(&mut *self, AtomSizeParse::Empty);
            match this {
                AtomSizeParse::Waiting(size, reader, opts) => {
                    *self = Self::Done(reader, opts);
                    #[cfg(not(feature = "extended_sized_atoms"))]
                    if size == 1 {
                        let _ = cx;
                        return core::task::Poll::Ready(Err(ParseError::AtomSizeUnsupported));
                    }

                    if size != 0 && size < AtomSize::MIN_ATOM_SIZE_32 {
                        return std::task::Poll::Ready(Err(ParseError::AtomSizeTooSmall))
                    }

                    let size = if size == 0 {
                        #[cfg(feature = "extended_sized_atoms")]
                        { u64::MAX }
                        #[cfg(not(feature = "extended_sized_atoms"))]
                        { u32::MAX }
                    } else {
                        #[cfg(feature = "extended_sized_atoms")]
                        {
                            u64::from(size - AtomSize::MIN_ATOM_SIZE_32)
                        }
                        #[cfg(not(feature = "extended_sized_atoms"))]
                        {
                            size - AtomSize::MIN_ATOM_SIZE_32
                        }
                    };

                    return core::task::Poll::Ready(Ok(AtomSize {
                        size
                    }))
                }
                #[cfg(feature = "extended_sized_atoms")]
                AtomSizeParse::ExtendedSize(mut fut) => {
                    match core::pin::Pin::new(&mut fut).poll(cx) {
                        core::task::Poll::Pending => {
                            *self = AtomSizeParse::ExtendedSize(fut);
                            return core::task::Poll::Pending;
                        }
                        core::task::Poll::Ready(res) => {
                            let IntegerParse::Done(reader, options) = fut else {
                                unreachable!("invalid future state");
                            };
                            *self = AtomSizeParse::Done(reader, options);
                            return std::task::Poll::Ready(res.and_then(|extended_size| {
                                if extended_size < AtomSize::MIN_ATOM_SIZE_64 {
                                    Err(ParseError::AtomSizeTooSmall)
                                } else {
                                    Ok(AtomSize {
                                        size: extended_size - AtomSize::MIN_ATOM_SIZE_64
                                    })
                                }
                            }))
                        }
                    }
                }
                AtomSizeParse::Done(..) => panic!("polled future after completion"),
                AtomSizeParse::Empty => unreachable!(),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for AtomSizeParse<'a, R> {
    impl_take_reader!{}

    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Waiting(_, r, opts) => (r, opts),
            #[cfg(feature = "extended_sized_atoms")]
            Self::ExtendedSize(s) => s.borrow_reader(),
            Self::Done(r, opts) => (r, opts),
            _ => unreachable!(),
        }
    }
}

