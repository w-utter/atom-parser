use crate::{
    AsyncParse, AsyncReadCstr, AsyncSeek, Extent, Offset, Parse, ParseError, ParseOptions, Parsed,
    PollReader, Reader, TakeReader, TryFromIntError, impl_take_reader,
};

mod pascal {
    use super::*;

    pub struct PascalString<O, S: Parsed> {
        len: <S as Parsed>::Output<O>,
        offset: O,
    }

    impl<O: core::fmt::Debug, S: Parsed> core::fmt::Debug for PascalString<O, S>
    where
        <S as Parsed>::Output<O>: core::fmt::Debug,
    {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("PascalString")
                .field("offset", &self.offset)
                .field("len", &self.len)
                .finish()
        }
    }

    impl<U, S: Parsed> Parsed for PascalString<U, S> {
        type Output<O> = PascalString<U, S>;
    }

    impl<O: Offset, S: Parsed> PascalString<O, S>
    where
        <S as Parsed>::Output<O>: Copy,
    {
        pub fn extent(&self) -> Extent<O, <S as Parsed>::Output<O>> {
            let Self { len, offset } = self;

            Extent {
                offset: *offset,
                len: *len,
            }
        }
    }

    impl<O: Offset, S: Parse<O>> Parse<O> for PascalString<O, S>
    where
        <S as Parsed>::Output<O>: TryInto<O> + Copy,
    {
        fn parse<T: Reader<O>>(
            reader: &mut T,
            options: &ParseOptions,
        ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
            let len = S::parse(reader, options)?;
            let offset = reader.offset();
            let length: O = len.try_into().map_err(|_| TryFromIntError)?;

            reader.seek(length)?;
            Ok(PascalString { len, offset })
        }
    }

    pub enum PascalStringParse<
        'a,
        O: Offset + Unpin,
        R: PollReader<O> + Unpin,
        S: AsyncParse<O> + Unpin,
    >
    where
        <S as AsyncParse<O>>::Fut<'a, R>: Unpin,
        <S as Parsed>::Output<O>: Unpin,
        <S as Parsed>::Output<O>: TryInto<O>,
    {
        Waiting(<S as AsyncParse<O>>::Fut<'a, R>),
        Seeking {
            offset: O,
            len: <S as Parsed>::Output<O>,
            fut: AsyncSeek<O, R>,
            opts: &'a ParseOptions,
        },
        Done(R, &'a ParseOptions),
        Empty,
    }

    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, S: AsyncParse<O> + Unpin + Copy> Future
        for PascalStringParse<'a, O, R, S>
    where
        <S as Parsed>::Output<O>: Unpin,
        <S as Parsed>::Output<O>: TryInto<O> + Copy,
    {
        type Output = Result<<PascalString<O, S> as Parsed>::Output<O>, ParseError>;
        fn poll(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            use core::pin::Pin;
            use core::task::Poll;
            loop {
                match core::mem::replace(&mut *self, Self::Empty) {
                    Self::Waiting(mut s) => match Pin::new(&mut s).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Waiting(s);
                            return Poll::Pending;
                        }
                        Poll::Ready(res) => {
                            let (reader, opts) = s.take_reader();
                            let res = match res {
                                Ok(res) => res,
                                Err(e) => {
                                    *self = Self::Done(reader, opts);
                                    return Poll::Ready(Err(e));
                                }
                            };

                            let len = res
                                .try_into()
                                .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;
                            let offset = reader.offset();

                            *self = Self::Seeking {
                                offset,
                                len: res,
                                fut: AsyncSeek::seek(reader, len),
                                opts,
                            }
                        }
                    },
                    Self::Seeking {
                        offset,
                        len,
                        mut fut,
                        opts,
                    } => match Pin::new(&mut fut).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Seeking {
                                offset,
                                len,
                                fut,
                                opts,
                            };
                            return Poll::Pending;
                        }
                        Poll::Ready(res) => {
                            let reader = fut.reader;
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(Ok(res.map(|_| PascalString { offset, len })?));
                        }
                    },
                    Self::Done(..) => panic!("polled after done"),
                    Self::Empty => unreachable!(),
                }
            }
        }
    }

    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, S: AsyncParse<O> + Unpin>
        TakeReader<'a, R> for PascalStringParse<'a, O, R, S>
    where
        <S as Parsed>::Output<O>: Unpin,
        <S as Parsed>::Output<O>: TryInto<O>,
    {
        impl_take_reader! {}
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            match self {
                Self::Waiting(w) => w.borrow_reader(),
                Self::Seeking { fut, opts, .. } => (&mut fut.reader, opts),
                Self::Done(r, opts) => (r, opts),
                _ => unreachable!(),
            }
        }
    }

    impl<O: Offset + Unpin, S: AsyncParse<O> + Unpin + Copy> AsyncParse<O> for PascalString<O, S>
    where
        <S as Parsed>::Output<O>: Unpin,
        <S as Parsed>::Output<O>: TryInto<O> + Copy,
    {
        type Fut<'a, R: PollReader<O> + Unpin> = PascalStringParse<'a, O, R, S>;
        fn create_fut<'a, R: PollReader<O> + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            PascalStringParse::Waiting(S::create_fut(reader, options))
        }
    }
}
pub use pascal::*;

mod null_terminated {
    use super::*;
    #[derive(Debug)]
    pub struct NullTerminatedString<O> {
        len: O,
        offset: O,
    }

    impl<U> Parsed for NullTerminatedString<U> {
        type Output<O> = NullTerminatedString<O>;
    }

    impl<O: Offset> NullTerminatedString<O> {
        pub fn extent(&self) -> Extent<O, O> {
            let Self { len, offset } = self;

            Extent {
                len: *len,
                offset: *offset,
            }
        }
    }

    impl<O: Offset> Parse<O> for NullTerminatedString<O> {
        fn parse<T: Reader<O>>(
            reader: &mut T,
            _: &ParseOptions,
        ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
            let offset = reader.offset();
            let len = reader.read_cstr()?;

            Ok(Self { offset, len })
        }
    }

    pub struct NullTerminatedStringParse<'a, O, R>(O, AsyncReadCstr<O, R>, &'a ParseOptions);

    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin> Future
        for NullTerminatedStringParse<'a, O, R>
    {
        type Output = Result<NullTerminatedString<O>, ParseError>;
        fn poll(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            use core::pin::Pin;
            use core::task::{Poll, ready};
            let len = ready!(Pin::new(&mut self.1).poll(cx))?;
            Poll::Ready(Ok(NullTerminatedString {
                offset: self.0,
                len,
            }))
        }
    }

    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin> TakeReader<'a, R>
        for NullTerminatedStringParse<'a, O, R>
    {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            (self.1.reader, self.2)
        }
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            (&mut self.1.reader, &self.2)
        }
    }

    impl<O: Offset + Unpin> AsyncParse<O> for NullTerminatedString<O> {
        type Fut<'a, R: PollReader<O> + Unpin> = NullTerminatedStringParse<'a, O, R>;
        fn create_fut<'a, R: PollReader<O> + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            let offset = reader.offset();
            NullTerminatedStringParse(offset, AsyncReadCstr::read_cstr(reader), options)
        }
    }
}
pub use null_terminated::*;
