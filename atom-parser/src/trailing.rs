use crate::{
    AsyncIterState, AsyncIterator, AsyncParse, BacktrackReader, Extent, NullTerminated, Offset,
    Options, Parse, ParseError, Parsed, PollReader, Reader, SwapOffsets, TakeReader,
    TrailingReader, TryFromIntError,
};

#[derive(Debug)]
pub struct Trailing<O, T, S = O> {
    offset: O,
    size: S,
    _pd: core::marker::PhantomData<T>,
}

impl<O: Offset, I: Parse<O>> Trailing<O, I> {
    pub fn parse<T: Reader<O>, P: Options>(reader: &mut T, _: P) -> Result<Self, ParseError> {
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
    pub fn parse_from_len<T: Reader<O>, P: Options>(
        size: S,
        reader: &mut T,
        _: P,
    ) -> Result<Self, ParseError> {
        let offset = reader.offset();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData,
        })
    }
}

pub struct TrailingParse<P, O: Unpin, R, T, S = O>(R, P, S, core::marker::PhantomData<(T, O)>);

impl<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O>, T: AsyncParse<O>, S: Unpin + Copy>
    Future for TrailingParse<P, O, R, T, S>
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

impl<P, O: Offset + Unpin, R, T: AsyncParse<O>, S: Unpin> TakeReader<P, R>
    for TrailingParse<P, O, R, T, S>
where
    <T as Parsed>::Output<O>: Unpin,
{
    fn take_reader(self) -> (R, P) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &P) {
        (&mut self.0, &self.1)
    }
}

impl<O: Offset + Unpin, T: AsyncParse<O>> Trailing<O, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn create_fut<P: Options + Unpin, R: PollReader<O> + Unpin>(
        reader: R,
        options: P,
    ) -> TrailingParse<P, O, R, T> {
        let size = reader.remaining_size();
        TrailingParse(reader, options, size, core::marker::PhantomData)
    }
}

impl<O: Offset + Unpin, T: AsyncParse<O>, S: Unpin> Trailing<O, T, S>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn create_fut_from_len<P: Options + Unpin, R: PollReader<O> + Unpin>(
        size: S,
        reader: R,
        options: P,
    ) -> TrailingParse<P, O, R, T, S> {
        TrailingParse(reader, options, size, core::marker::PhantomData)
    }
}

pub struct TrailingIterator<P, O, R: SwapOffsets<O>, T> {
    reader: BacktrackReader<O, TrailingReader<O, R>>,
    opts: P,
    _pd: core::marker::PhantomData<T>,
}

impl<P, O: Offset, R: SwapOffsets<O> + Reader<O>, T> TrailingIterator<P, O, R, T> {
    pub fn from_trailing<S: Copy + TryInto<O>>(
        trailing: &Trailing<O, T, S>,
        reader: R,
        opts: P,
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

    pub fn from_null_terminated(
        null_terminated: &NullTerminated<O, T>,
        reader: R,
        opts: P,
    ) -> Self {
        let Extent { offset, len } = null_terminated.extent();
        let max_offset = offset + len;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), offset);
        Self {
            reader,
            opts,
            _pd: core::marker::PhantomData,
        }
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<O, TrailingReader<O, R>> {
        &mut self.reader
    }
}

impl<P: Options, O: Offset, T: Parse<O>, R: Reader<O>> Iterator for TrailingIterator<P, O, R, T> {
    type Item = Result<<T as Parsed>::Output<O>, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining_size() == O::ZERO {
            return None;
        }

        let item = T::parse(&mut self.reader, &self.opts);
        Some(item)
    }
}

pub struct AsyncTrailingIterator<
    P: Options + Unpin,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    T: AsyncParse<O>,
> where
    <T as Parsed>::Output<O>: Unpin,
{
    state: AsyncIterState<
        P,
        BacktrackReader<O, TrailingReader<O, R>>,
        T::Fut<P, BacktrackReader<O, TrailingReader<O, R>>>,
    >,
}

impl<
    P: Options + Unpin,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    T: AsyncParse<O>,
> AsyncTrailingIterator<P, O, R, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    pub fn from_trailing<S: Copy + TryInto<O>>(
        trailing: &Trailing<O, T, S>,
        reader: R,
        opts: P,
    ) -> Result<Self, ParseError> {
        let Trailing { offset, size, .. } = trailing;

        let size = (*size)
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        let max_offset = offset.add(size);
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);

        let state = if PollReader::remaining_size(&reader) > O::ZERO {
            AsyncIterState::Iterating(T::create_fut(reader, opts))
        } else {
            AsyncIterState::Done(reader, opts)
        };
        Ok(Self { state })
    }

    pub fn from_null_terminated(trailing: &NullTerminated<O, T>, reader: R, opts: P) -> Self {
        let Extent { offset, len } = trailing.extent();
        let max_offset = offset + len;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), offset);

        let state = if PollReader::remaining_size(&reader) > O::ZERO {
            AsyncIterState::Iterating(T::create_fut(reader, opts))
        } else {
            AsyncIterState::Done(reader, opts)
        };
        Self { state }
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<O, TrailingReader<O, R>> {
        self.borrow_reader().0
    }
}

impl<
    P: Options + Unpin,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    T: AsyncParse<O>,
> AsyncIterator for AsyncTrailingIterator<P, O, R, T>
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
                    self.state = if reader.remaining_size() > O::ZERO {
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

impl<
    P: Options + Unpin,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    T: AsyncParse<O>,
> TakeReader<P, BacktrackReader<O, TrailingReader<O, R>>> for AsyncTrailingIterator<P, O, R, T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    fn take_reader(self) -> (BacktrackReader<O, TrailingReader<O, R>>, P) {
        self.state.take_reader()
    }
    fn borrow_reader(&mut self) -> (&mut BacktrackReader<O, TrailingReader<O, R>>, &P) {
        self.state.borrow_reader()
    }
}
