use crate::{
    AsyncIterState, AsyncIterator, AsyncParse, BacktrackReader, Parse, ParseError, ParseOptions,
    PollReader, Reader, SwapOffsets, TakeReader, TrailingReader, impl_take_reader,
};

#[derive(Debug)]
pub struct Children<T> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: usize,
}

#[derive(Debug)]
pub struct SizedChildren<S, T> {
    _pd: core::marker::PhantomData<T>,
    pub(crate) len: S,
    pub(crate) offset: usize,
}

mod sync_impl {
    use super::*;

    impl<S: Parse + Unpin, I: Parse + Unpin> Parse for SizedChildren<S, I> {
        fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
            let len = S::parse(reader, options)?;
            let offset = reader.offset();
            Ok(Self {
                _pd: core::marker::PhantomData,
                len,
                offset,
            })
        }
    }

    impl<I: Parse + Unpin> Parse for Children<I> {
        fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
            let offset = reader.offset();
            let size = reader.remaining_size();
            Ok(Self {
                _pd: core::marker::PhantomData,
                offset,
                size,
            })
        }
    }

    pub struct ChildrenIter<'a, R: SwapOffsets, C> {
        _pd: core::marker::PhantomData<C>,
        pub reader: BacktrackReader<TrailingReader<&'a mut R>>,
        opts: &'a ParseOptions,
    }

    impl<'a, R: SwapOffsets + Reader, C> ChildrenIter<'a, R, C> {
        pub fn from_children(
            children: &Children<C>,
            reader: &'a mut R,
            opts: &'a ParseOptions,
        ) -> Self {
            let Children { offset, size, .. } = children;

            let max_offset = offset + size;
            let reader =
                BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset);
            Self {
                reader,
                opts,
                _pd: core::marker::PhantomData,
            }
        }
    }

    impl<'a, R: Reader, C: Parse> Iterator for ChildrenIter<'a, R, C> {
        type Item = Result<C, ParseError>;
        fn next(&mut self) -> Option<Self::Item> {
            if self.reader.remaining_size() > 0 {
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
    pub struct AsyncChildrenIter<'a, R: SwapOffsets + PollReader + Unpin, C: AsyncParse> {
        state: AsyncIterState<
            'a,
            BacktrackReader<TrailingReader<&'a mut R>>,
            <C as AsyncParse>::Fut<'a, BacktrackReader<TrailingReader<&'a mut R>>>,
        >,
    }

    impl<'a, R: SwapOffsets + PollReader + Unpin + Reader, C: AsyncParse> AsyncChildrenIter<'a, R, C> {
        pub fn from_children(
            children: &Children<C>,
            reader: &'a mut R,
            opts: &'a ParseOptions,
        ) -> Self {
            let Children { offset, size, .. } = children;

            let max_offset = offset + size;
            let reader =
                BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset);
            let remaining = PollReader::remaining_size(&reader);

            let state = if remaining > 0 {
                AsyncIterState::Iterating(C::create_fut(reader, opts))
            } else {
                AsyncIterState::Done(reader, opts)
            };

            Self { state }
        }

        pub fn reader(&mut self) -> &mut BacktrackReader<TrailingReader<&'a mut R>> {
            let (reader, _) = self.state.borrow_reader();
            reader
        }
    }

    impl<'a, R: SwapOffsets + PollReader + Unpin, C: AsyncParse + Unpin> AsyncIterator
        for AsyncChildrenIter<'a, R, C>
    where
        <C as AsyncParse>::Fut<'a, R>: Unpin,
    {
        type Item = Result<C, ParseError>;
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
                        self.state = if reader.remaining_size() > 0 {
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

    impl<'a, R: SwapOffsets + PollReader + Unpin, C: AsyncParse + Unpin>
        TakeReader<'a, BacktrackReader<TrailingReader<&'a mut R>>> for AsyncChildrenIter<'a, R, C>
    where
        <C as AsyncParse>::Fut<'a, R>: Unpin,
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

    pub enum SizedChildrenParse<'a, R: PollReader + Unpin, S: AsyncParse + Unpin, C>
    where
        <S as AsyncParse>::Fut<'a, R>: Unpin,
    {
        Size {
            fut: <S as AsyncParse>::Fut<'a, R>,
            _pd: core::marker::PhantomData<C>,
        },
        Done(R, &'a ParseOptions),
        Empty,
    }

    impl<'a, R: PollReader + Unpin, S: AsyncParse + Unpin, C: AsyncParse + Unpin> Future
        for SizedChildrenParse<'a, R, S, C>
    where
        <S as AsyncParse>::Fut<'a, R>: Unpin,
    {
        type Output = Result<SizedChildren<S, C>, ParseError>;
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

    impl<'a, R: PollReader + Unpin, S: AsyncParse + Unpin, C: AsyncParse> TakeReader<'a, R>
        for SizedChildrenParse<'a, R, S, C>
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

    impl<S: AsyncParse + Unpin, C: AsyncParse + Unpin> AsyncParse for SizedChildren<S, C> {
        type Fut<'a, R: PollReader + Unpin> = SizedChildrenParse<'a, R, S, C>;
        fn create_fut<'a, R: PollReader + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            SizedChildrenParse::Size {
                fut: S::create_fut(reader, options),
                _pd: core::marker::PhantomData,
            }
        }
    }

    pub struct ChildrenParse<'a, R, C>(R, &'a ParseOptions, core::marker::PhantomData<C>);
    impl<'a, R: PollReader + Unpin, C: AsyncParse> Future for ChildrenParse<'a, R, C> {
        type Output = Result<Children<C>, ParseError>;
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

    impl<'a, R: PollReader + Unpin, C: AsyncParse> TakeReader<'a, R> for ChildrenParse<'a, R, C> {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            (self.0, self.1)
        }
        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
            (&mut self.0, self.1)
        }
    }

    impl<C: AsyncParse + Unpin> AsyncParse for Children<C> {
        type Fut<'a, R: PollReader + Unpin> = ChildrenParse<'a, R, C>;
        fn create_fut<'a, R: PollReader + Unpin>(
            reader: R,
            options: &'a ParseOptions,
        ) -> Self::Fut<'a, R> {
            ChildrenParse(reader, options, core::marker::PhantomData)
        }
    }
}
pub use async_impl::*;
