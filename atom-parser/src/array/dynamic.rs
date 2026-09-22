use crate::{
    AsyncIterState, AsyncIterator, AsyncParse, BacktrackReader, Offset, Parse, ParseError,
    ParseOptions, Parsed, PollReader, Reader, SizedChildren, SwapOffsets, TakeReader,
    impl_take_reader,
};

pub trait ArraySize: Clone + Copy + core::ops::AddAssign + core::cmp::Ord {
    const ZERO: Self;
    const ONE: Self;
}

macro_rules! impl_array_size {
    ($($i:ty),*,) => {
        $(
            impl ArraySize for $i {
                const ZERO: Self = 0;
                const ONE: Self = 1;
            }
        )*

        pub trait TryFromArraySize: $(TryFrom<$i>+)+ {}

        impl  <T: $(TryFrom<$i>+)+> TryFromArraySize for T { }
    }
}

impl_array_size! {
    u8, u16, u32,
}

pub struct DynamicArray<O, S: Parsed, I, const ZERO_RELATIVE: bool> {
    size: <S as Parsed>::Output<O>,
    offset: O,
    _pd: core::marker::PhantomData<I>,
}

impl<U, S: Parsed, I, const ZERO_RELATIVE: bool> Parsed for DynamicArray<U, S, I, ZERO_RELATIVE> {
    type Output<O> = DynamicArray<U, S, I, ZERO_RELATIVE>;
}

impl<O: core::fmt::Debug, S: Parsed, I, const ZERO_RELATIVE: bool> core::fmt::Debug
    for DynamicArray<O, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DynamicArray")
            .field("offset", &self.offset)
            .field("size", &self.size)
            .finish()
    }
}

pub struct DynamicArrayIter<'a, O, R: SwapOffsets<O>, S: Parsed, I, const ZERO_RELATIVE: bool> {
    size: <S as Parsed>::Output<O>,
    current: <S as Parsed>::Output<O>,
    exhausted: bool,
    pub reader: BacktrackReader<O, &'a mut R>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<I>,
}

impl<'a, O: Offset, R: SwapOffsets<O>, S: Parsed, I, const ZERO_RELATIVE: bool>
    DynamicArrayIter<'a, O, R, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: ArraySize,
{
    pub fn from_dynamic_array(
        arr: &DynamicArray<O, S, I, ZERO_RELATIVE>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Self {
        let DynamicArray { offset, size, .. } = arr;
        Self::new(*size, reader, *offset, opts)
    }

    pub fn from_sized_children(
        children: &SizedChildren<O, S, I>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Self {
        let SizedChildren { len, offset, .. } = children;
        Self::new(*len, reader, *offset, opts)
    }

    fn new(
        size: <S as Parsed>::Output<O>,
        reader: &'a mut R,
        offset: O,
        opts: &'a ParseOptions,
    ) -> Self {
        Self {
            size,
            current: <<S as Parsed>::Output<O>>::ZERO,
            exhausted: false,
            reader: BacktrackReader::new(reader, offset),
            opts,
            _pd: core::marker::PhantomData,
        }
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<O, &'a mut R> {
        &mut self.reader
    }
}

fn dynamic_array_iter_has_next<const ZERO_RELATIVE: bool, S: ArraySize>(
    current: &mut S,
    max_size: &S,
    exhausted: &mut bool,
) -> bool {
    let has_next = if ZERO_RELATIVE {
        // inclusive range
        if *exhausted {
            false
        } else {
            *current <= *max_size
        }
    } else {
        // exclusive range
        *current < *max_size
    };

    if !has_next {
        return false;
    }

    if ZERO_RELATIVE && current == max_size {
        *exhausted = true;
    } else {
        *current += S::ONE;
    }
    true
}

impl<'a, O: Offset, R: Reader<O>, S: Parsed, I: Parse<O>, const ZERO_RELATIVE: bool> Iterator
    for DynamicArrayIter<'a, O, R, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: ArraySize,
{
    type Item = Result<<I as Parsed>::Output<O>, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        let has_next = dynamic_array_iter_has_next::<ZERO_RELATIVE, _>(
            &mut self.current,
            &self.size,
            &mut self.exhausted,
        );

        if !has_next {
            return None;
        }
        Some(I::parse(&mut self.reader, &self.opts))
    }
}

pub struct AsyncDynamicArrayIter<
    'a,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    S: Parsed,
    I: AsyncParse<O>,
    const ZERO_RELATIVE: bool,
> where
    <I as Parsed>::Output<O>: Unpin,
{
    size: <S as Parsed>::Output<O>,
    current: <S as Parsed>::Output<O>,
    exhausted: bool,

    state: AsyncIterState<
        'a,
        BacktrackReader<O, &'a mut R>,
        I::Fut<'a, BacktrackReader<O, &'a mut R>>,
    >,
}

impl<
    'a,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    S: Parsed,
    I: AsyncParse<O>,
    const ZERO_RELATIVE: bool,
> AsyncDynamicArrayIter<'a, O, R, S, I, ZERO_RELATIVE>
where
    <I as Parsed>::Output<O>: Unpin,
    <S as Parsed>::Output<O>: ArraySize + Unpin,
{
    pub fn from_dynamic_array(
        arr: &DynamicArray<O, S, I, ZERO_RELATIVE>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Self {
        let DynamicArray { offset, size, .. } = arr;
        Self::new(*size, reader, *offset, opts)
    }

    pub fn from_sized_children(
        children: &SizedChildren<O, S, <I as Parsed>::Output<O>>,
        reader: &'a mut R,
        opts: &'a ParseOptions,
    ) -> Self {
        let SizedChildren { len, offset, .. } = children;
        Self::new(*len, reader, *offset, opts)
    }

    fn new(
        size: <S as Parsed>::Output<O>,
        reader: &'a mut R,
        offset: O,
        opts: &'a ParseOptions,
    ) -> Self {
        let reader = BacktrackReader::new(reader, offset);

        let mut exhausted = false;
        let mut current = <<S as Parsed>::Output<O> as ArraySize>::ZERO;

        let state =
            if dynamic_array_iter_has_next::<ZERO_RELATIVE, _>(&mut current, &size, &mut exhausted)
            {
                AsyncIterState::Iterating(I::create_fut(reader, opts))
            } else {
                AsyncIterState::Done(reader, opts)
            };

        Self {
            size,
            current,
            exhausted,
            state,
        }
    }

    pub fn reader(&mut self) -> &mut BacktrackReader<O, &'a mut R> {
        let (reader, _) = self.state.borrow_reader();
        reader
    }
}

impl<
    'a,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    S: Parsed,
    I: AsyncParse<O> + Unpin,
    const ZERO_RELATIVE: bool,
> AsyncIterator for AsyncDynamicArrayIter<'a, O, R, S, I, ZERO_RELATIVE>
where
    <I as Parsed>::Output<O>: Unpin,
    <S as Parsed>::Output<O>: ArraySize + Unpin,
{
    type Item = Result<<I as Parsed>::Output<O>, ParseError>;
    fn poll_next(
        mut self: core::pin::Pin<&mut Self>,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        use core::pin::Pin;
        use core::task::Poll;

        let mut this = core::mem::replace(
            &mut *self,
            Self {
                size: <<S as Parsed>::Output<O>>::ZERO,
                current: <<S as Parsed>::Output<O>>::ZERO,
                exhausted: false,
                state: AsyncIterState::Empty,
            },
        );
        match this.state {
            AsyncIterState::Done(r, o) => {
                this.state = AsyncIterState::Done(r, o);
                core::mem::swap(&mut *self, &mut this);
                return Poll::Ready(None);
            }
            AsyncIterState::Iterating(mut i) => match Pin::new(&mut i).poll(ctx) {
                Poll::Pending => {
                    this.state = AsyncIterState::Iterating(i);
                    core::mem::swap(&mut *self, &mut this);
                    return Poll::Pending;
                }
                Poll::Ready(res) => {
                    let (reader, opts) = i.take_reader();
                    this.state = if dynamic_array_iter_has_next::<ZERO_RELATIVE, _>(
                        &mut this.current,
                        &this.size,
                        &mut this.exhausted,
                    ) {
                        AsyncIterState::Iterating(I::create_fut(reader, opts))
                    } else {
                        AsyncIterState::Done(reader, opts)
                    };
                    core::mem::swap(&mut *self, &mut this);
                    return Poll::Ready(Some(res));
                }
            },
            _ => unreachable!(),
        }
    }
}

impl<
    'a,
    O: Offset + Unpin,
    R: SwapOffsets<O> + PollReader<O> + Unpin,
    S: Parsed,
    I: AsyncParse<O> + Unpin,
    const ZERO_RELATIVE: bool,
> TakeReader<'a, BacktrackReader<O, &'a mut R>>
    for AsyncDynamicArrayIter<'a, O, R, S, I, ZERO_RELATIVE>
where
    <I as Parsed>::Output<O>: Unpin,
    <S as Parsed>::Output<O>: ArraySize + Unpin,
{
    fn take_reader(self) -> (BacktrackReader<O, &'a mut R>, &'a ParseOptions) {
        self.state.take_reader()
    }
    fn borrow_reader(&mut self) -> (&mut BacktrackReader<O, &'a mut R>, &'a ParseOptions) {
        self.state.borrow_reader()
    }
}

impl<O: Offset, I: Parse<O> + Unpin, S: Parse<O> + Unpin, const ZERO_RELATIVE: bool> Parse<O>
    for DynamicArray<O, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: ArraySize,
{
    fn parse<T: Reader<O>>(
        reader: &mut T,
        options: &ParseOptions,
    ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
        let size = S::parse(reader, options)?;
        let offset = reader.offset();
        let mut current = <<S as Parsed>::Output<O>>::ZERO;
        let mut exhausted = false;

        while dynamic_array_iter_has_next::<ZERO_RELATIVE, _>(&mut current, &size, &mut exhausted) {
            let _ = I::parse(reader, options)?;
        }

        Ok(Self {
            size,
            offset,
            _pd: core::marker::PhantomData,
        })
    }
}

pub enum DynamicArrayParse<
    'a,
    O: Offset + Unpin,
    R: PollReader<O> + Unpin,
    S: AsyncParse<O>,
    I: AsyncParse<O>,
    const ZERO_RELATIVE: bool,
> where
    <S as Parsed>::Output<O>: ArraySize + Unpin,
    <I as Parsed>::Output<O>: Unpin,
{
    Waiting(<S as AsyncParse<O>>::Fut<'a, R>),
    Iterating {
        offset: O,
        exhausted: bool,
        len: <S as Parsed>::Output<O>,
        idx: <S as Parsed>::Output<O>,
        fut: <I as AsyncParse<O>>::Fut<'a, R>,
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl<
    'a,
    O: Offset + Unpin,
    R: PollReader<O> + Unpin,
    S: AsyncParse<O> + Unpin,
    I: AsyncParse<O> + Unpin,
    const ZERO_RELATIVE: bool,
> Future for DynamicArrayParse<'a, O, R, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: ArraySize + Unpin,
    <I as Parsed>::Output<O>: Unpin,
{
    type Output = Result<DynamicArray<O, S, I, ZERO_RELATIVE>, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Waiting(mut fut) => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Waiting(fut);
                        return Poll::Pending;
                    }
                    Poll::Ready(s) => {
                        let (reader, options) = fut.take_reader();
                        let offset = reader.offset();

                        let len = match s {
                            Ok(s) => s,
                            Err(e) => {
                                *self = Self::Done(reader, options);
                                return Poll::Ready(Err(e));
                            }
                        };
                        let mut idx = <<S as Parsed>::Output<O>>::ZERO;
                        let mut exhausted = false;

                        if dynamic_array_iter_has_next::<ZERO_RELATIVE, _>(
                            &mut idx,
                            &len,
                            &mut exhausted,
                        ) {
                            *self = Self::Iterating {
                                offset,
                                len,
                                idx,
                                exhausted,
                                fut: I::create_fut(reader, options),
                            }
                        } else {
                            *self = Self::Done(reader, options);
                            return Poll::Ready(Ok(DynamicArray {
                                offset,
                                size: len,
                                _pd: core::marker::PhantomData,
                            }));
                        }
                    }
                },
                Self::Iterating {
                    offset,
                    mut exhausted,
                    len,
                    mut idx,
                    mut fut,
                } => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Iterating {
                            offset,
                            exhausted,
                            len,
                            idx,
                            fut,
                        }
                    }
                    Poll::Ready(i) => {
                        let (reader, opts) = fut.take_reader();
                        if let Err(e) = i {
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(Err(e));
                        }

                        let has_next = dynamic_array_iter_has_next::<ZERO_RELATIVE, _>(
                            &mut idx,
                            &len,
                            &mut exhausted,
                        );
                        if has_next {
                            *self = Self::Iterating {
                                offset,
                                exhausted,
                                len,
                                idx,
                                fut: I::create_fut(reader, opts),
                            };
                            continue;
                        }
                        *self = Self::Done(reader, opts);
                        return Poll::Ready(Ok(DynamicArray {
                            offset,
                            size: len,
                            _pd: core::marker::PhantomData,
                        }));
                    }
                },
                Self::Done(..) => panic!("polled after completion"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl<
    'a,
    O: Offset + Unpin,
    R: PollReader<O> + Unpin,
    S: AsyncParse<O>,
    I: AsyncParse<O>,
    const ZERO_RELATIVE: bool,
> TakeReader<'a, R> for DynamicArrayParse<'a, O, R, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: ArraySize + Unpin,
    <I as Parsed>::Output<O>: Unpin,
{
    impl_take_reader! {}
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Waiting(f) => f.borrow_reader(),
            Self::Iterating { fut, .. } => fut.borrow_reader(),
            Self::Done(r, opts) => (r, opts),
            Self::Empty => unreachable!(),
        }
    }
}

impl<
    O: Offset + Unpin,
    S: AsyncParse<O> + Unpin,
    I: AsyncParse<O> + Unpin,
    const ZERO_RELATIVE: bool,
> AsyncParse<O> for DynamicArray<O, S, I, ZERO_RELATIVE>
where
    <S as Parsed>::Output<O>: ArraySize + Unpin,
    <I as Parsed>::Output<O>: Unpin,
{
    type Fut<'a, R: PollReader<O> + Unpin> = DynamicArrayParse<'a, O, R, S, I, ZERO_RELATIVE>;
    fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R> {
        DynamicArrayParse::Waiting(S::create_fut(reader, options))
    }
}
