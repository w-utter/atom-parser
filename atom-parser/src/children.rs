use crate::{
    AsyncIterState, AsyncIterator, AsyncParse, BacktrackReader, FourCC, Offset, Parse, ParseError,
    ParseOptions, Parsed, PollReader, Reader, SwapOffsets, TakeReader, TrailingReader,
    impl_take_reader,
};

#[derive(Debug)]
pub struct Children<O, T> {
    _pd: core::marker::PhantomData<T>,
    offset: O,
    size: O,
}

impl<U, T: Parsed> Parsed for Children<U, T> {
    type Output<O> = Children<U, <T as Parsed>::Output<O>>;
}

pub struct SizedChildren<O, S: Parsed, T> {
    _pd: core::marker::PhantomData<T>,
    pub(crate) len: <S as Parsed>::Output<O>,
    pub(crate) offset: O,
}

impl<O: core::fmt::Debug, S: Parsed, T> core::fmt::Debug for SizedChildren<O, S, T>
where
    <S as Parsed>::Output<O>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SizedChildren")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .finish()
    }
}

impl<U, S: Parsed, T: Parsed> Parsed for SizedChildren<U, S, T> {
    type Output<O> = SizedChildren<U, S, <T as Parsed>::Output<O>>;
}

mod sync_impl {
    use super::*;

    impl<O: Offset, S: Parse<O>, I: Parse<O>> Parse<O> for SizedChildren<O, S, I> {
        fn parse<T: Reader<O>>(
            reader: &mut T,
            options: &ParseOptions,
        ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
            let len = S::parse(reader, options)?;
            let offset = reader.offset();
            Ok(SizedChildren {
                _pd: core::marker::PhantomData,
                len,
                offset,
            })
        }
    }

    impl<O: Offset, I: Parse<O>> Parse<O> for Children<O, I> {
        fn parse<T: Reader<O>>(
            reader: &mut T,
            _: &ParseOptions,
        ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
            let offset = reader.offset();
            let size = reader.remaining_size();
            Ok(Children {
                _pd: core::marker::PhantomData,
                offset,
                size,
            })
        }
    }

    pub struct ChildrenIter<'a, O, R: SwapOffsets<O>, C> {
        _pd: core::marker::PhantomData<C>,
        pub reader: BacktrackReader<O, TrailingReader<O, &'a mut R>>,
        opts: &'a ParseOptions,
    }

    impl<'a, O: Offset, R: SwapOffsets<O> + Reader<O>, C> ChildrenIter<'a, O, R, C> {
        pub fn from_children(
            children: &Children<O, C>,
            reader: &'a mut R,
            opts: &'a ParseOptions,
        ) -> Self {
            let Children { offset, size, .. } = children;

            let max_offset = offset.add(*size);
            let reader =
                BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset);
            Self {
                reader,
                opts,
                _pd: core::marker::PhantomData,
            }
        }
    }

    impl<'a, O: Offset, R: Reader<O>, C: Parse<O>> Iterator for ChildrenIter<'a, O, R, C> {
        type Item = Result<<C as Parsed>::Output<O>, ParseError>;
        fn next(&mut self) -> Option<Self::Item> {
            if self.reader.remaining_size() > O::zero() {
                Some(C::parse(&mut self.reader, self.opts))
            } else {
                None
            }
        }
    }
}
pub use sync_impl::*;

mod async_impl {
    use super::*;
    pub struct AsyncChildrenIter<
        'a,
        O: Offset + Unpin,
        R: SwapOffsets<O> + PollReader<O> + Unpin,
        C: AsyncParse<O>,
    >
    where
        <C as Parsed>::Output<O>: Unpin,
    {
        state: AsyncIterState<
            'a,
            BacktrackReader<O, TrailingReader<O, &'a mut R>>,
            <C as AsyncParse<O>>::Fut<'a, BacktrackReader<O, TrailingReader<O, &'a mut R>>>,
        >,
    }

    impl<'a, O: Offset + Unpin, R: SwapOffsets<O> + PollReader<O> + Unpin, C: AsyncParse<O>>
        AsyncChildrenIter<'a, O, R, C>
    where
        <C as Parsed>::Output<O>: Unpin,
    {
        pub fn from_children(
            children: &Children<O, <C as Parsed>::Output<O>>,
            reader: &'a mut R,
            opts: &'a ParseOptions,
        ) -> Self {
            let Children { offset, size, .. } = children;

            let max_offset = offset.add(*size);
            let reader =
                BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset);
            let remaining = PollReader::remaining_size(&reader);

            let state = if remaining > O::zero() {
                AsyncIterState::Iterating(C::create_fut(reader, opts))
            } else {
                AsyncIterState::Done(reader, opts)
            };

            Self { state }
        }

        pub fn reader(&mut self) -> &mut BacktrackReader<O, TrailingReader<O, &'a mut R>> {
            let (reader, _) = self.state.borrow_reader();
            reader
        }
    }

    impl<
        'a,
        O: Offset + Unpin + Copy,
        R: SwapOffsets<O> + PollReader<O> + Unpin,
        C: AsyncParse<O> + Unpin,
    > AsyncIterator for AsyncChildrenIter<'a, O, R, C>
    where
        <C as Parsed>::Output<O>: Unpin,
    {
        type Item = Result<<C as Parsed>::Output<O>, ParseError>;
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
                AsyncIterState::Done(r, o) => {
                    self.state = AsyncIterState::Done(r, o);
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
                            AsyncIterState::Iterating(C::create_fut(reader, opts))
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

    impl<'a, O: Offset + Unpin, R: SwapOffsets<O> + PollReader<O> + Unpin, C: AsyncParse<O> + Unpin>
        TakeReader<'a, BacktrackReader<O, TrailingReader<O, &'a mut R>>>
        for AsyncChildrenIter<'a, O, R, C>
    where
        <C as Parsed>::Output<O>: Unpin,
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

    pub enum SizedChildrenParse<
        'a,
        O: Offset + Unpin,
        R: PollReader<O> + Unpin,
        S: AsyncParse<O> + Unpin,
        C,
    >
    where
        <S as Parsed>::Output<O>: Unpin,
    {
        Size {
            fut: <S as AsyncParse<O>>::Fut<'a, R>,
            _pd: core::marker::PhantomData<C>,
        },
        Done(R, &'a ParseOptions),
        Empty,
    }

    impl<
        'a,
        O: Offset + Unpin,
        R: PollReader<O> + Unpin,
        S: AsyncParse<O> + Unpin,
        C: AsyncParse<O> + Unpin,
    > Future for SizedChildrenParse<'a, O, R, S, C>
    where
        <S as Parsed>::Output<O>: Unpin,
        <C as Parsed>::Output<O>: Unpin,
    {
        type Output = Result<<SizedChildren<O, S, C> as Parsed>::Output<O>, ParseError>;
        fn poll(
            mut self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            use core::pin::Pin;
            use core::task::Poll;
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Size { mut fut, .. } => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Size {
                            fut,
                            _pd: core::marker::PhantomData,
                        };
                        return Poll::Pending;
                    }
                    Poll::Ready(size) => {
                        let (reader, options) = fut.take_reader();
                        let offset = reader.offset();
                        *self = Self::Done(reader, options);
                        return Poll::Ready(size.map(|len| SizedChildren {
                            offset,
                            len,
                            _pd: core::marker::PhantomData,
                        }));
                    }
                },
                Self::Done(..) => panic!("poll after completion"),
                Self::Empty => unreachable!(),
            }
        }
    }

    impl<
        'a,
        O: Offset + Unpin,
        R: PollReader<O> + Unpin,
        S: AsyncParse<O> + Unpin,
        C: AsyncParse<O>,
    > TakeReader<'a, R> for SizedChildrenParse<'a, O, R, S, C>
    where
        <S as Parsed>::Output<O>: Unpin,
        <C as Parsed>::Output<O>: Unpin,
    {
        impl_take_reader! {}
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            match self {
                Self::Size { fut, .. } => fut.borrow_reader(),
                Self::Done(r, opts) => (r, opts),
                Self::Empty => unreachable!(),
            }
        }
    }

    impl<O: Offset + Unpin, S: AsyncParse<O> + Unpin, C: AsyncParse<O> + Unpin> AsyncParse<O>
        for SizedChildren<O, S, C>
    where
        <S as Parsed>::Output<O>: Unpin,
        <C as Parsed>::Output<O>: Unpin,
    {
        type Fut<'a, R: PollReader<O> + Unpin> = SizedChildrenParse<'a, O, R, S, C>;
        fn create_fut<'a, R: PollReader<O> + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            SizedChildrenParse::Size {
                fut: S::create_fut(reader, options),
                _pd: core::marker::PhantomData,
            }
        }
    }

    pub struct ChildrenParse<'a, O: Offset + Unpin, R, C>(
        R,
        &'a ParseOptions,
        core::marker::PhantomData<(O, C)>,
    );
    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, C: AsyncParse<O>> Future
        for ChildrenParse<'a, O, R, C>
    where
        <C as Parsed>::Output<O>: Unpin,
    {
        type Output = Result<<Children<O, C> as Parsed>::Output<O>, ParseError>;
        fn poll(
            self: core::pin::Pin<&mut Self>,
            _: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            let offset = self.0.offset();
            let size = self.0.remaining_size();
            core::task::Poll::Ready(Ok(Children {
                offset,
                size,
                _pd: core::marker::PhantomData,
            }))
        }
    }

    impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, C: AsyncParse<O>> TakeReader<'a, R>
        for ChildrenParse<'a, O, R, C>
    where
        <C as Parsed>::Output<O>: Unpin,
    {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            (self.0, self.1)
        }
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            (&mut self.0, self.1)
        }
    }

    impl<O: Offset + Unpin, C: AsyncParse<O> + Unpin> AsyncParse<O> for Children<O, C>
    where
        <C as Parsed>::Output<O>: Unpin,
    {
        type Fut<'a, R: PollReader<O> + Unpin> = ChildrenParse<'a, O, R, C>;
        fn create_fut<'a, R: PollReader<O> + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            ChildrenParse(reader, options, core::marker::PhantomData)
        }
    }
}
pub use async_impl::*;

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownChild<O> {
    pub child_fcc: FourCC,
    _offset: core::marker::PhantomData<O>,
}

impl<O> UnknownChild<O> {
    pub fn new(child_fcc: FourCC) -> Self {
        Self {
            child_fcc,
            _offset: core::marker::PhantomData,
        }
    }
}
