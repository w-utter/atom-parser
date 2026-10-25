use crate::{
    AsyncParse, Extent, Offset, Parse, ParseError, ParseOptions, Parsed, PollReader, Reader,
    TakeReader, impl_take_reader,
};

pub struct NullTerminated<O, T> {
    offset: O,
    size: O,
    _pd: core::marker::PhantomData<T>,
}

impl<O: Copy, T> NullTerminated<O, T> {
    pub fn extent(&self) -> Extent<O, O> {
        let Self { offset, size, .. } = self;
        Extent {
            offset: *offset,
            len: *size,
        }
    }
}

impl<U, T> Parsed for NullTerminated<U, T> {
    type Output<O> = NullTerminated<U, T>;
}

impl<O: Offset, I: Parse<O>> Parse<O> for NullTerminated<O, I>
where
    <I as Parsed>::Output<O>: PartialEq,
{
    fn parse<T: Reader<O>>(
        reader: &mut T,
        options: &ParseOptions,
    ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
        let offset = reader.offset();
        let mut end;
        loop {
            end = reader.offset();
            let item = I::parse(reader, options)?;
            if item == unsafe { core::mem::zeroed() } {
                break;
            }
        }

        Ok(NullTerminated {
            offset,
            size: end - offset,
            _pd: core::marker::PhantomData,
        })
    }
}

pub enum AsyncNullTerminatedParse<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, I: AsyncParse<O>>
where
    <I as Parsed>::Output<O>: Unpin,
{
    Iterating {
        offset: O,
        end: O,
        fut: <I as AsyncParse<O>>::Fut<'a, R>,
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, I: AsyncParse<O>> Future
    for AsyncNullTerminatedParse<'a, O, R, I>
where
    <I as Parsed>::Output<O>: PartialEq + Unpin,
{
    type Output = Result<NullTerminated<O, I>, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;

        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Iterating {
                    offset,
                    mut end,
                    mut fut,
                } => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Iterating { offset, end, fut };
                        return Poll::Pending;
                    }
                    Poll::Ready(i) => {
                        let (reader, opts) = fut.take_reader();
                        let item = match i {
                            Ok(i) => i,
                            Err(e) => {
                                *self = Self::Done(reader, opts);
                                return Poll::Ready(Err(e));
                            }
                        };

                        if item == unsafe { core::mem::zeroed() } {
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(Ok(NullTerminated {
                                offset,
                                size: end - offset,
                                _pd: core::marker::PhantomData,
                            }));
                        }

                        end = reader.offset();

                        *self = Self::Iterating {
                            offset,
                            end,
                            fut: I::create_fut(reader, opts),
                        };
                    }
                },
                Self::Done(..) => panic!("poll after future complete"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, I: AsyncParse<O>> TakeReader<'a, R>
    for AsyncNullTerminatedParse<'a, O, R, I>
where
    <I as Parsed>::Output<O>: Unpin,
{
    impl_take_reader! {}
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Iterating { fut, .. } => fut.borrow_reader(),
            Self::Done(reader, opts) => (reader, opts),
            Self::Empty => unreachable!(),
        }
    }
}

impl<O: Offset + Unpin, I: AsyncParse<O> + Unpin> AsyncParse<O> for NullTerminated<O, I>
where
    <I as Parsed>::Output<O>: Unpin + PartialEq,
{
    type Fut<'a, R: PollReader<O> + Unpin> = AsyncNullTerminatedParse<'a, O, R, I>;
    fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R> {
        let offset = reader.offset();
        let end = reader.offset();
        AsyncNullTerminatedParse::Iterating {
            offset,
            end,
            fut: I::create_fut(reader, options),
        }
    }
}
