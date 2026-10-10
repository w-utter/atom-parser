use crate::{
    AsyncParse, Endianess, Offset, Parse, ParseError, ParseOptions, Parsed, PollReader, Reader,
    TakeReader, impl_take_reader,
};

pub enum IntegerParse<'a, O, R, T, const N: usize> {
    Waiting(
        R,
        &'a ParseOptions,
        [u8; N],
        core::marker::PhantomData<(O, T)>,
    ),
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, O, R, T, const N: usize> TakeReader<'a, R> for IntegerParse<'a, O, R, T, N> {
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

            impl Parsed for $i {
                type Output<O> = $i;
            }

            impl <O: Offset> Parse<O> for $i {
                fn parse<T: Reader<O>>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
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

            impl <O: Offset + Unpin> AsyncParse<O> for $i {
                type Fut<'a, R: PollReader<O> + Unpin> = IntegerParse<'a, O, R, $i, { core::mem::size_of::<$i>() }>;
                fn create_fut<'a, R: PollReader<O> + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    IntegerParse::Waiting(reader, options, [0; _], core::marker::PhantomData)
                }
            }

            impl <'a, O: Offset + Unpin, R: PollReader<O> + Unpin> Future for IntegerParse<'a, O, R, $i, { core::mem::size_of::<$i>() }> {
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
                                    return core::task::Poll::Ready(Ok(res.map(|_| if matches!(opts.endianess, Endianess::Big) {
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

// bool is treated as a special case of u8,
// where its 7 most significant bits are reserved
mod bool_impl {
    use super::*;
    impl Parsed for bool {
        type Output<O> = bool;
    }

    impl<O: Offset> Parse<O> for bool {
        fn parse<T: Reader<O>>(reader: &mut T, opts: &ParseOptions) -> Result<Self, ParseError> {
            let byte = u8::parse(reader, opts)?;
            if opts.error_on_used_reserved_fields && (byte & !1) != 0 {
                return Err(ParseError::UsedReservedField);
            }
            Ok(byte & 1 == 1)
        }
    }

    pub struct AsyncBoolParse<'a, O, R> {
        integer: IntegerParse<'a, O, R, u8, 1>,
    }

    impl<'a, O, R> TakeReader<'a, R> for AsyncBoolParse<'a, O, R> {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            self.integer.take_reader()
        }

        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            self.integer.borrow_reader()
        }
    }

    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin> Future for AsyncBoolParse<'a, O, R> {
        type Output = ::core::result::Result<bool, ParseError>;
        fn poll(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            let mut this = core::mem::replace(
                &mut *self,
                Self {
                    integer: IntegerParse::Empty,
                },
            );
            match core::pin::Pin::new(&mut this.integer).poll(cx) {
                core::task::Poll::Pending => {
                    *self = this;
                    return core::task::Poll::Pending;
                }
                core::task::Poll::Ready(r) => {
                    *self = this;
                    let byte = r?;

                    let (_, opts) = self.borrow_reader();

                    if opts.error_on_used_reserved_fields && (byte & !1) != 0 {
                        return core::task::Poll::Ready(Err(ParseError::UsedReservedField));
                    }
                    return core::task::Poll::Ready(Ok(byte & 1 == 1));
                }
            }
        }
    }

    impl<O: Offset + Unpin> AsyncParse<O> for bool {
        type Fut<'a, R: PollReader<O> + Unpin> = AsyncBoolParse<'a, O, R>;
        fn create_fut<'a, R: PollReader<O> + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            AsyncBoolParse {
                integer: u8::create_fut(reader, options),
            }
        }
    }
}
pub use bool_impl::*;
