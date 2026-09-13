use crate::{Extent, ParseOptions, impl_take_reader, Parse, Reader, PollReader, AsyncParse, TakeReader, TryFromIntError, AsyncSeek, ParseError, AsyncReadCstr};

mod pascal {
    use super::*;

    #[derive(Debug)]
    pub struct PascalString<S> {
        len: S,
        offset: usize,
    }

    impl <S: Copy> PascalString<S> {
        pub fn extent(&self) -> Extent<usize, S> {
            let Self {
                len,
                offset,
            } = self;

            Extent {
                offset: *offset,
                len: *len,
            }
        }
    }

    impl <S: Parse + TryInto<usize> + Copy + Unpin> Parse for PascalString<S> {
        fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
            let len = S::parse(reader, options)?;
            let offset = reader.offset();
            let length: usize = len.try_into().map_err(|_| TryFromIntError)?;

            reader.seek(length)?;
            Ok(Self {
                len,
                offset,
            })
        }
    }

    pub enum PascalStringParse<'a, R: PollReader + Unpin, S: AsyncParse + TryInto<usize> + Unpin> where <S as AsyncParse>::Fut<'a, R>: Unpin {
        Waiting(<S as AsyncParse>::Fut<'a, R>),
        Seeking{
            offset: usize, 
            len: S, 
            fut: AsyncSeek<R>, 
            opts: &'a ParseOptions
        },
        Done(R, &'a ParseOptions),
        Empty,
    }

    impl <'a, R: PollReader + Unpin, S: AsyncParse + TryInto<usize> + Unpin + Copy> Future for PascalStringParse<'a, R, S> where <S as AsyncParse>::Fut<'a, R>: Unpin {
        type Output = Result<PascalString<S>, ParseError>;
        fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
            use core::pin::Pin;
            use core::task::Poll;
            loop {
                match core::mem::replace(&mut *self, Self::Empty) {
                    Self::Waiting(mut s) => {
                        match Pin::new(&mut s).poll(cx) {
                            Poll::Pending => {
                                *self = Self::Waiting(s);
                                return Poll::Pending;
                            }
                            Poll::Ready(res) => {
                                let res = res?;
                                let (reader, opts) = s.take_reader();
                                let len = res.try_into().map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;
                                let offset = reader.offset();

                                *self = Self::Seeking {
                                    offset,
                                    len: res,
                                    fut: AsyncSeek::seek(reader, len),
                                    opts,
                                }
                            }
                        }
                    }
                    Self::Seeking {
                        offset,
                        len,
                        mut fut,
                        opts,
                    } => {
                        match Pin::new(&mut fut).poll(cx) {
                            Poll::Pending => {
                                *self = Self::Seeking{
                                    offset,
                                    len, 
                                    fut, 
                                    opts
                                };
                                return Poll::Pending;
                            }
                            Poll::Ready(res) => {
                                let reader = fut.reader;
                                *self = Self::Done(reader, opts);
                                return Poll::Ready(Ok(res.map(|_| PascalString {
                                    offset,
                                    len,
                                })?))
                            }
                        }
                    }
                    Self::Done(..) => panic!("polled after done"),
                    Self::Empty => unreachable!(),
                }
            }
        }
    }

    impl <'a, R: PollReader + Unpin, S: AsyncParse + TryInto<usize> + Unpin> TakeReader<'a, R> for PascalStringParse<'a, R, S> where <S as AsyncParse>::Fut<'a, R>: Unpin {
        impl_take_reader!{}
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            match self {
                Self::Waiting(w) => w.borrow_reader(),
                Self::Seeking {
                    fut,
                    opts,
                    ..
                } => (&mut fut.reader, opts),
                Self::Done(r, opts) => (r, opts),
                _ => unreachable!(),
            }
        }
    }

    impl <S: AsyncParse + TryInto<usize> + Unpin + Copy> AsyncParse for PascalString<S> {
        type Fut<'a, R: PollReader + Unpin> = PascalStringParse<'a, R, S>;
        fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
            PascalStringParse::Waiting(S::create_fut(reader, options))
        }
    }
}
pub use pascal::*;

mod null_terminated {
    use super::*;
    #[derive(Debug)]
    pub struct NullTerminatedString {
        len: usize,
        offset: usize,
    }

    impl NullTerminatedString {
        pub fn extent(&self) -> Extent<usize, usize> {
            let Self {
                len,
                offset,
            } = self;

            Extent {
                len: *len,
                offset: *offset,
            }
        }
    }

    impl Parse for NullTerminatedString {
        fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
            let offset = reader.offset();
            let len = reader.read_cstr()?;

            Ok(Self {
                offset,
                len,
            })
        }
    }

    pub struct NullTerminatedStringParse<'a, R>(usize, AsyncReadCstr<R>, &'a ParseOptions);

    impl <'a, R: PollReader + Unpin> Future for NullTerminatedStringParse<'a, R> {
        type Output = Result<NullTerminatedString, ParseError>;
        fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
            use core::task::{Poll, ready};
            use core::pin::Pin;
            let len = ready!(Pin::new(&mut self.1).poll(cx))?;
            Poll::Ready(Ok(NullTerminatedString {
                offset: self.0,
                len,
            }))
        }
    }

    impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for NullTerminatedStringParse<'a, R> {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            (self.1.reader, self.2)
        }
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            (&mut self.1.reader, &self.2)
        }
    }

    impl AsyncParse for NullTerminatedString {
        type Fut<'a, R: PollReader + Unpin> = NullTerminatedStringParse<'a, R>;
        fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
            let offset = reader.offset();
            NullTerminatedStringParse(offset, AsyncReadCstr::read_cstr(reader), options)
        }
    }
}
pub use null_terminated::*;
