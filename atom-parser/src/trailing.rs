use crate::{
    AsyncIterState, AsyncIterator, AsyncParse, BacktrackReader, Parse, ParseError, ParseOptions,
    PollReader, Reader, SwapOffsets, TakeReader, TrailingReader, TryFromIntError,
};

#[derive(Debug)]
pub struct Trailing<T, S = usize> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: S,
}

impl<I: Parse + Unpin> Trailing<I> {
    pub fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData,
        })
    }
}

impl<I: Parse + Unpin, S> Trailing<I, S> {
    pub fn parse_from_len<T: Reader>(
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

pub struct TrailingParse<'a, R, T, S = usize>(R, &'a ParseOptions, S, core::marker::PhantomData<T>);

impl<'a, R: PollReader, T: AsyncParse, S: Unpin + Copy> Future for TrailingParse<'a, R, T, S> {
    type Output = Result<Trailing<T, S>, ParseError>;
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

impl<'a, R, T: AsyncParse, S: Unpin> TakeReader<'a, R> for TrailingParse<'a, R, T, S> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        (&mut self.0, self.1)
    }
}

impl<T: AsyncParse + Unpin> Trailing<T> {
    pub fn create_fut<'a, R: PollReader + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> TrailingParse<'a, R, T> {
        let size = reader.remaining_size();
        TrailingParse(reader, options, size, core::marker::PhantomData)
    }
}

impl<T: AsyncParse + Unpin, S: Unpin> Trailing<T, S> {
    pub fn create_fut_from_len<'a, R: PollReader + Unpin>(
        size: S,
        reader: R,
        options: &'a ParseOptions,
    ) -> TrailingParse<'a, R, T, S> {
        TrailingParse(reader, options, size, core::marker::PhantomData)
    }
}

pub struct TrailingIterator<'a, R: SwapOffsets, T> {
    reader: BacktrackReader<TrailingReader<&'a mut R>>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<T>,
}

impl<'a, R: SwapOffsets + Reader, T> TrailingIterator<'a, R, T> {
    pub fn from_trailing<S: Copy + TryInto<usize>>(
        trailing: &Trailing<T, S>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Result<Self, ParseError> {
        let Trailing { offset, size, .. } = trailing;

        let size = (*size)
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        let max_offset = offset + size;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);
        Ok(Self {
            reader,
            opts,
            _pd: core::marker::PhantomData,
        })
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<TrailingReader<&'a mut R>> {
        &mut self.reader
    }
}

impl<'a, T: Parse, R: Reader> Iterator for TrailingIterator<'a, R, T> {
    type Item = Result<T, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining_size() == 0 {
            return None;
        }

        let item = T::parse(&mut self.reader, self.opts);
        Some(item)
    }
}

pub struct AsyncTrailingIterator<'a, R: SwapOffsets + PollReader + Unpin, T: AsyncParse + Unpin> {
    state: AsyncIterState<
        'a,
        BacktrackReader<TrailingReader<&'a mut R>>,
        T::Fut<'a, BacktrackReader<TrailingReader<&'a mut R>>>,
    >,
}

impl<'a, R: SwapOffsets + PollReader + Unpin + Reader, T: AsyncParse + Unpin>
    AsyncTrailingIterator<'a, R, T>
{
    pub fn from_trailing<S: Copy + TryInto<usize>>(
        trailing: &Trailing<T, S>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Result<Self, ParseError> {
        let Trailing { offset, size, .. } = trailing;

        let size = (*size)
            .try_into()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;

        let max_offset = offset + size;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);

        let state = if PollReader::remaining_size(&reader) == 0 {
            AsyncIterState::Done(reader, opts)
        } else {
            AsyncIterState::Iterating(T::create_fut(reader, opts))
        };
        Ok(Self { state })
    }
}

impl<'a, R: SwapOffsets + PollReader + Unpin, T: AsyncParse + Unpin> AsyncIterator
    for AsyncTrailingIterator<'a, R, T>
{
    type Item = Result<T, ParseError>;
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
                    self.state = if reader.remaining_size() == 0 {
                        AsyncIterState::Done(reader, opts)
                    } else {
                        AsyncIterState::Iterating(T::create_fut(reader, opts))
                    };
                    return Poll::Ready(Some(res));
                }
            },
            _ => unreachable!(),
        }
    }
}

impl<'a, R: SwapOffsets + PollReader + Unpin, T: AsyncParse + Unpin>
    TakeReader<'a, BacktrackReader<TrailingReader<&'a mut R>>> for AsyncTrailingIterator<'a, R, T>
{
    fn take_reader(self) -> (BacktrackReader<TrailingReader<&'a mut R>>, &'a ParseOptions) {
        self.state.take_reader()
    }
    fn borrow_reader(
        &mut self,
    ) -> (
        &mut BacktrackReader<TrailingReader<&'a mut R>>,
        &'a ParseOptions,
    ) {
        self.state.borrow_reader()
    }
}
