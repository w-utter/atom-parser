use crate::{
    AsyncParse, Endianess, Offset, Options, Parse, ParseError, Parsed, PollReader, Reader,
    TakeReader, impl_take_reader,
};

pub enum IntegerParse<P, O, R, T, const N: usize> {
    Waiting(R, P, [u8; N], core::marker::PhantomData<(O, T)>),
    Done(R, P),
    Empty,
}

impl<P: Options, O, R, T, const N: usize> TakeReader<P, R> for IntegerParse<P, O, R, T, N> {
    impl_take_reader! {}

    fn borrow_reader(&mut self) -> (&mut R, &P) {
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
                fn parse<T: Reader<O>, P: Options>(reader: &mut T, options: P) -> Result<Self, ParseError> {
                    let mut buf = [0; core::mem::size_of::<$i>()];
                    reader.read(&mut buf)?;

                    Ok(if matches!(options.endianess(), Endianess::Big) {
                        // likely path
                        <$i>::from_be_bytes(buf)
                    } else {
                        <$i>::from_le_bytes(buf)
                    })
                }
            }

            impl <O: Offset + Unpin> AsyncParse<O> for $i {
                type Fut<P: Options + Unpin, R: PollReader<O> + Unpin> = IntegerParse<P, O, R, $i, { core::mem::size_of::<$i>() }>;
                fn create_fut<P: Options + Unpin, R: PollReader<O> + Unpin>(reader: R, options: P) -> Self::Fut<P, R> {
                    IntegerParse::Waiting(reader, options, [0; _], core::marker::PhantomData)
                }
            }

            impl <P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> Future for IntegerParse<P, O, R, $i, { core::mem::size_of::<$i>() }> {
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
                                    let is_big_endian = matches!(opts.endianess(), Endianess::Big);
                                    *self = Self::Done(r, opts);
                                    return core::task::Poll::Ready(Ok(res.map(|_| if is_big_endian {
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
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

// bool is treated as a special case of u8,
// where its 7 most significant bits are reserved
mod bool_impl {
    use super::*;
    impl Parsed for bool {
        type Output<O> = bool;
    }

    impl<O: Offset> Parse<O> for bool {
        fn parse<T: Reader<O>, P: Options>(reader: &mut T, opts: P) -> Result<Self, ParseError> {
            let byte = u8::parse(reader, &opts)?;
            if opts.options().error_on_used_reserved_fields && (byte & !1) != 0 {
                return Err(ParseError::UsedReservedField);
            }
            Ok(byte & 1 == 1)
        }
    }

    pub struct AsyncBoolParse<P, O, R> {
        integer: IntegerParse<P, O, R, u8, 1>,
    }

    impl<P: Options, O, R> TakeReader<P, R> for AsyncBoolParse<P, O, R> {
        fn take_reader(self) -> (R, P) {
            self.integer.take_reader()
        }

        fn borrow_reader(&mut self) -> (&mut R, &P) {
            self.integer.borrow_reader()
        }
    }

    impl<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> Future
        for AsyncBoolParse<P, O, R>
    {
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

                    if opts.options().error_on_used_reserved_fields && (byte & !1) != 0 {
                        return core::task::Poll::Ready(Err(ParseError::UsedReservedField));
                    }
                    return core::task::Poll::Ready(Ok(byte & 1 == 1));
                }
            }
        }
    }

    impl<O: Offset + Unpin> AsyncParse<O> for bool {
        type Fut<P: Options + Unpin, R: PollReader<O> + Unpin> = AsyncBoolParse<P, O, R>;
        fn create_fut<P: Options + Unpin, R: PollReader<O> + Unpin>(
            reader: R,
            options: P,
        ) -> Self::Fut<P, R> {
            AsyncBoolParse {
                integer: u8::create_fut(reader, options),
            }
        }
    }
}
pub use bool_impl::*;
