use crate::{
    AsyncParse, AsyncSeek, Offset, Parse, ParseError, ParseOptions, Parsed, PollReader, Reader,
    TakeReader, TrailingReader, TryFromIntError, impl_take_reader,
};

pub struct Padded<O, S, T> {
    _pd: core::marker::PhantomData<(O, S, T)>,
}

impl<U, S, T: Parsed> Parsed for Padded<U, S, T> {
    type Output<O> = <T as Parsed>::Output<U>;
}

// cant use parse here since it needs a size
impl<O: Offset, S: TryInto<O>, T: Parse<O>> Padded<O, S, T> {
    pub fn parse_from_len<R: Reader<O>>(
        size: S,
        reader: &mut R,
        opts: &ParseOptions,
    ) -> Result<<T as Parsed>::Output<O>, ParseError> {
        let offset = reader.offset();
        let size: O = size
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        let max_offset = offset + size;
        let mut r = TrailingReader::new(reader, max_offset);
        let inner = T::parse(&mut r, opts)?;
        r.seek_remaining()?;
        Ok(inner)
    }
}

pub enum AsyncPaddedParse<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, S, T: AsyncParse<O>>
where
    <T as Parsed>::Output<O>: Unpin,
{
    Setup(O, S, R, &'a ParseOptions),
    ParsingInner(<T as AsyncParse<O>>::Fut<'a, TrailingReader<O, R>>),
    SeekRemaining(
        <T as Parsed>::Output<O>,
        AsyncSeek<O, TrailingReader<O, R>>,
        &'a ParseOptions,
    ),
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, S: TryInto<O> + Unpin, T: AsyncParse<O>>
    Future for AsyncPaddedParse<'a, O, R, S, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    type Output = Result<<T as Parsed>::Output<O>, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Setup(offset, size, reader, opts) => {
                    let size = match size.try_into() {
                        Ok(size) => size,
                        Err(_) => {
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(Err(ParseError::IntegerConversion(
                                TryFromIntError,
                            )));
                        }
                    };

                    let max_offset = offset + size;
                    let r = TrailingReader::new(reader, max_offset);
                    *self = Self::ParsingInner(T::create_fut(r, opts));
                }
                Self::ParsingInner(mut fut) => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::ParsingInner(fut);
                        return Poll::Pending;
                    }
                    Poll::Ready(f) => {
                        let (reader, opts) = fut.take_reader();
                        let inner = match f {
                            Ok(inner) => inner,
                            Err(e) => {
                                *self = Self::Done(reader.reader, opts);
                                return Poll::Ready(Err(e));
                            }
                        };
                        let remaining = reader.remaining_size();
                        let seek = AsyncSeek::seek(reader, remaining);
                        *self = Self::SeekRemaining(inner, seek, opts);
                    }
                },
                Self::SeekRemaining(inner, mut seek, opts) => match Pin::new(&mut seek).poll(cx) {
                    Poll::Pending => {
                        *self = Self::SeekRemaining(inner, seek, opts);
                        return Poll::Pending;
                    }
                    Poll::Ready(res) => {
                        *self = Self::Done(seek.reader.reader, opts);
                        let _ = res?;
                        return Poll::Ready(Ok(inner));
                    }
                },
                Self::Done(..) => panic!("poll after future complete"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, S, T: AsyncParse<O>> TakeReader<'a, R>
    for AsyncPaddedParse<'a, O, R, S, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    impl_take_reader! {}

    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Setup(_, _, reader, opts) => (reader, opts),
            Self::ParsingInner(fut) => {
                let (reader, opts) = fut.borrow_reader();
                (&mut reader.reader, opts)
            }
            Self::SeekRemaining(_, seek, opts) => (&mut seek.reader.reader, opts),
            Self::Done(reader, opts) => (reader, opts),
            Self::Empty => unreachable!(),
        }
    }
}

impl<O: Offset + Unpin, S: TryInto<O> + Unpin, T: AsyncParse<O>> Padded<O, S, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn create_fut_from_len<'a, R: PollReader<O> + Unpin>(
        size: S,
        reader: R,
        opts: &'a ParseOptions,
    ) -> AsyncPaddedParse<'a, O, R, S, T> {
        let offset = reader.offset();
        AsyncPaddedParse::Setup(offset, size, reader, opts)
    }
}
