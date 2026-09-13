use crate::{Parse, Reader, ParseOptions, PollReader, AsyncParse, TakeReader, ParseError, SwapOffsets, TrailingReader, BacktrackReader, AsyncIterator, AsyncIterState};

#[derive(Debug)]
pub struct Trailing<T> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: usize,
}

impl <I: Parse + Unpin> Parse for Trailing<I> {
    fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData
        })
    }
}

pub struct TrailingParse<'a, R, T>(R, &'a ParseOptions, core::marker::PhantomData<T>);

impl <'a, R: PollReader, T: AsyncParse> Future for TrailingParse<'a, R, T> {
    type Output = Result<Trailing<T>, ParseError>;
    fn poll(self: core::pin::Pin<&mut Self>, _: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.0.remaining_size();
        core::task::Poll::Ready(Ok(Trailing {
            offset,
            size,
            _pd: core::marker::PhantomData,
        }))
    }
}

impl <'a, R, T: AsyncParse> TakeReader<'a, R> for TrailingParse<'a, R, T> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        (&mut self.0, self.1)
    }
}

impl <T: AsyncParse + Unpin> AsyncParse for Trailing<T> {
    type Fut<'a, R: PollReader + Unpin> = TrailingParse<'a, R, T>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        TrailingParse(reader, options, core::marker::PhantomData)
    }
}

pub struct TrailingIterator<'a, R: SwapOffsets, T> {
    reader: BacktrackReader<TrailingReader<&'a mut R>>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<T>,
}

impl <'a, R: SwapOffsets + Reader, T> TrailingIterator<'a, R, T> {
    pub fn from_trailing(trailing: &Trailing<T>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let Trailing {
            offset,
            size,
            ..
        } = trailing;

        let max_offset = offset + size;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);
        Self {
            reader,
            opts,
            _pd: core::marker::PhantomData,
        }
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<TrailingReader<&'a mut R>> {
        &mut self.reader
    }
}

impl <'a, T: Parse, R: Reader> Iterator for TrailingIterator<'a, R, T> {
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
    state: AsyncIterState<'a, BacktrackReader<TrailingReader<&'a mut R>>, T::Fut<'a, BacktrackReader<TrailingReader<&'a mut R>>>>,
}

impl <'a, R: SwapOffsets + PollReader + Unpin + Reader, T: AsyncParse + Unpin> AsyncTrailingIterator<'a, R, T> {
    pub fn from_trailing(trailing: &Trailing<T>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let Trailing {
            offset,
            size,
            ..
        } = trailing;

        let max_offset = offset + size;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), *offset);

        let state = if PollReader::remaining_size(&reader) == 0 {
            AsyncIterState::Done(reader, opts)
        } else {
            AsyncIterState::Iterating(T::create_fut(reader, opts))
        };
        Self {
            state
        }
    }
}

impl <'a, R: SwapOffsets + PollReader + Unpin, T: AsyncParse + Unpin> AsyncIterator for AsyncTrailingIterator<'a, R, T> {
    type Item = Result<T, ParseError>;
    fn poll_next(mut self: core::pin::Pin<&mut Self>, ctx: &mut core::task::Context<'_>) -> core::task::Poll<Option<Self::Item>> {
        use core::pin::Pin;
        use core::task::Poll;

        let state = core::mem::replace(&mut *self, Self { state: AsyncIterState::Empty }).state;
        match state {
            AsyncIterState::Done(r, opts) => {
                self.state = AsyncIterState::Done(r, opts);
                return Poll::Ready(None)
            }
            AsyncIterState::Iterating(mut i) => {
                match Pin::new(&mut i).poll(ctx) {
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
                        return Poll::Ready(Some(res))
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

impl <'a, R: SwapOffsets + PollReader + Unpin, T: AsyncParse + Unpin> TakeReader<'a, BacktrackReader<TrailingReader<&'a mut R>>> for AsyncTrailingIterator<'a, R, T> {
    fn take_reader(self) -> (BacktrackReader<TrailingReader<&'a mut R>>, &'a ParseOptions) {
        self.state.take_reader()
    }
    fn borrow_reader(&mut self) -> (&mut BacktrackReader<TrailingReader<&'a mut R>>, &'a ParseOptions) {
        self.state.borrow_reader()
    }
}

