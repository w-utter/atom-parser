use crate::{
    AsyncIterState, AsyncIterator, AsyncParse, BacktrackReader, Offset, Parse, ParseError,
    ParseOptions, Parsed, PollReader, Reader, SwapOffsets, TakeReader, TrailingReader,
    TryFromIntError,
};

#[derive(Debug)]
pub struct Trailing<O, T, S = O> {
    offset: O,
    size: S,
    _pd: core::marker::PhantomData<T>,
}

impl<O: Offset, I: Parse<O>> Trailing<O, I> {
    pub fn parse<T: Reader<O>>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData,
        })
    }
}

impl<O: Offset, I: Parse<O>, S> Trailing<O, I, S> {
    pub fn parse_from_len<T: Reader<O>>(
        size: S,
        reader: &mut T,
        _: &ParseOptions,
    ) -> Result<Self, ParseError> {
        let offset = reader.offset();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData,
        })
    }
}

pub struct TrailingParse<'a, O: Unpin, R, T, S = O>(
    R,
    &'a ParseOptions,
    S,
    core::marker::PhantomData<(T, O)>,
);

impl<'a, O: Offset + Unpin, R: PollReader<O>, T: AsyncParse<O>, S: Unpin + Copy> Future
    for TrailingParse<'a, O, R, T, S>
where
    <T as Parsed>::Output<O>: Unpin,
{
    type Output = Result<Trailing<O, T, S>, ParseError>;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.2;
        core::task::Poll::Ready(Ok(Trailing {
            offset,
            size,
            _pd: core::marker::PhantomData,
        }))
    }
}

impl<'a, O: Offset + Unpin, R, T: AsyncParse<O>, S: Unpin> TakeReader<'a, R>
    for TrailingParse<'a, O, R, T, S>
where
    <T as Parsed>::Output<O>: Unpin,
{
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        (&mut self.0, self.1)
    }
}

impl<O: Offset + Unpin, T: AsyncParse<O> + Unpin> Trailing<O, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> TrailingParse<'a, O, R, T> {
        let size = reader.remaining_size();
        TrailingParse(reader, options, size, core::marker::PhantomData)
    }
}

impl<O: Offset + Unpin, T: AsyncParse<O> + Unpin, S: Unpin> Trailing<O, T, S>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn create_fut_from_len<'a, R: PollReader<O> + Unpin>(
        size: S,
        reader: R,
        options: &'a ParseOptions,
    ) -> TrailingParse<'a, O, R, T, S> {
        TrailingParse(reader, options, size, core::marker::PhantomData)
    }
}

pub struct TrailingIterator<'a, O, R: SwapOffsets<O>, T> {
    reader: BacktrackReader<O, TrailingReader<O, &'a mut R>>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<T>,
}

impl<'a, O: Offset, R: SwapOffsets<O> + Reader<O>, T> TrailingIterator<'a, O, R, T> {
    pub fn from_trailing<S: Copy + TryInto<O>>(
        trailing: &Trailing<O, T, S>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Result<Self, ParseError> {
        let Trailing { offset, size, .. } = trailing;

        let size = (*size)
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        let max_offset = offset.add(size);

        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);
        Ok(Self {
            reader,
            opts,
            _pd: core::marker::PhantomData,
        })
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<O, TrailingReader<O, &'a mut R>> {
        &mut self.reader
    }
}

impl<'a, O: Offset, T: Parse<O>, R: Reader<O>> Iterator for TrailingIterator<'a, O, R, T> {
    type Item = Result<<T as Parsed>::Output<O>, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining_size() == O::zero() {
            return None;
        }

        let item = T::parse(&mut self.reader, self.opts);
        Some(item)
    }
}

pub struct AsyncTrailingIterator<
    'a,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    T: AsyncParse<O> + Unpin,
> where
    <T as Parsed>::Output<O>: Unpin,
{
    state: AsyncIterState<
        'a,
        BacktrackReader<O, TrailingReader<O, &'a mut R>>,
        T::Fut<'a, BacktrackReader<O, TrailingReader<O, &'a mut R>>>,
    >,
}

impl<'a, O: Offset + Unpin, R: SwapOffsets<O> + PollReader<O> + Unpin, T: AsyncParse<O> + Unpin>
    AsyncTrailingIterator<'a, O, R, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn from_trailing<S: Copy + TryInto<O>>(
        trailing: &Trailing<O, T, S>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Result<Self, ParseError> {
        let Trailing { offset, size, .. } = trailing;

        let size = (*size)
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        let max_offset = offset.add(size);
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);

        let state = if PollReader::remaining_size(&reader) > O::zero() {
            AsyncIterState::Iterating(T::create_fut(reader, opts))
        } else {
            AsyncIterState::Done(reader, opts)
        };
        Ok(Self { state })
    }
}

impl<'a, O: Offset + Unpin, R: SwapOffsets<O> + PollReader<O> + Unpin, T: AsyncParse<O> + Unpin>
    AsyncIterator for AsyncTrailingIterator<'a, O, R, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    type Item = Result<<T as Parsed>::Output<O>, ParseError>;
    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        use core::pin::Pin;
        use core::task::Poll;

        let state = core::mem::replace(
            &mut *self,
            Self {
                state: AsyncIterState::Empty,
            },
        )
        .state;
        match state {
            AsyncIterState::Done(r, opts) => {
                self.state = AsyncIterState::Done(r, opts);
                return Poll::Ready(None);
            }
            AsyncIterState::Iterating(mut i) => match Pin::new(&mut i).poll(ctx) {
                Poll::Pending => {
                    self.state = AsyncIterState::Iterating(i);
                    return Poll::Pending;
                }
                Poll::Ready(res) => {
                    let (reader, opts) = i.take_reader();
                    self.state = if reader.remaining_size() > O::zero() {
                        AsyncIterState::Iterating(T::create_fut(reader, opts))
                    } else {
                        AsyncIterState::Done(reader, opts)
                    };
                    return Poll::Ready(Some(res));
                }
            },
            _ => unreachable!(),
        }
    }
}

impl<'a, O: Offset + Unpin, R: SwapOffsets<O> + PollReader<O> + Unpin, T: AsyncParse<O> + Unpin>
    TakeReader<'a, BacktrackReader<O, TrailingReader<O, &'a mut R>>>
    for AsyncTrailingIterator<'a, O, R, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    fn take_reader(
        self,
    ) -> (
        BacktrackReader<O, TrailingReader<O, &'a mut R>>,
        &'a ParseOptions,
    ) {
        self.state.take_reader()
    }
    fn borrow_reader(
        &mut self,
    ) -> (
        &mut BacktrackReader<O, TrailingReader<O, &'a mut R>>,
        &'a ParseOptions,
    ) {
        self.state.borrow_reader()
    }
}
