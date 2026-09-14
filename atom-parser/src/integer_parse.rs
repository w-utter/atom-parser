use crate::{
    AsyncParse, Endianess, Parse, ParseError, ParseOptions, PollReader, Reader, TakeReader,
    impl_take_reader,
};

pub enum IntegerParse<'a, R, T, const N: usize> {
    Waiting(R, &'a ParseOptions, [u8; N], core::marker::PhantomData<T>),
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, R, T, const N: usize> TakeReader<'a, R> for IntegerParse<'a, R, T, N> {
    impl_take_reader! {}

    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Waiting(r, opts, ..) => (r, opts),
            Self::Done(r, opts) => (r, opts),
            _ => unreachable!(),
        }
    }
}

macro_rules! parse_integers {
    ($($i:ty),*,) => {
        $(
            impl Parse for $i {
                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                    let mut buf = [0; core::mem::size_of::<$i>()];
                    reader.read(&mut buf)?;

                    Ok(if matches!(options.endianess, Endianess::Big) {
                        // likely path
                        <$i>::from_be_bytes(buf)
                    } else {
                        <$i>::from_le_bytes(buf)
                    })
                }
            }

            impl AsyncParse for $i {
                type Fut<'a, R: PollReader + Unpin> = IntegerParse<'a, R, $i, { core::mem::size_of::<$i>() }>;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    IntegerParse::Waiting(reader, options, [0; _], core::marker::PhantomData)
                }
            }

            impl <'a, R: PollReader + Unpin> Future for IntegerParse<'a, R, $i, { core::mem::size_of::<$i>() }> {
                type Output = Result<$i, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    let this = core::mem::replace(&mut *self, IntegerParse::Empty);
                    match this {
                        IntegerParse::Waiting(mut r, opts, mut buf, _) => {
                            match core::pin::Pin::new(&mut r).poll_read(cx, &mut buf) {
                                core::task::Poll::Pending => {
                                    *self = Self::Waiting(r, opts, buf, core::marker::PhantomData);
                                    return core::task::Poll::Pending
                                }
                                core::task::Poll::Ready(res) => {
                                    *self = Self::Done(r, opts);
                                    return std::task::Poll::Ready(Ok(res.map(|_| if matches!(opts.endianess, Endianess::Big) {
                                        // likely path
                                        <$i>::from_be_bytes(buf)
                                    } else {
                                        <$i>::from_le_bytes(buf)
                                    })?))
                                }
                            }
                        }
                        IntegerParse::Done(..) => panic!("future polled after completion"),
                        IntegerParse::Empty => unreachable!(),
                    }
                }
            }
        )*
    }
}

parse_integers! {
    u8, u16, u32, u64,
    i8, i16, i32, i64,
}
