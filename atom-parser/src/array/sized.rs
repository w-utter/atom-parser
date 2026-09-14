use crate::{
    AsyncIterState, AsyncParse, Parse, ParseError, ParseOptions, PollReader, Reader, TakeReader,
};

pub struct ArrayGuard<'a, T, const N: usize> {
    arr: &'a mut [core::mem::MaybeUninit<T>; N],
    initialized: usize,
}

impl<'a, T, const N: usize> ArrayGuard<'a, T, N> {
    pub fn new(arr: &'a mut [core::mem::MaybeUninit<T>; N]) -> Self {
        Self {
            arr,
            initialized: 0,
        }
    }

    pub unsafe fn push(&mut self, item: T) {
        debug_assert!(self.initialized < N, "trying to write past array");
        unsafe {
            let uninit = self.arr.get_unchecked_mut(self.initialized);
            uninit.write(item);
        }
        self.initialized += 1;
    }

    pub fn uninit() -> [core::mem::MaybeUninit<T>; N] {
        [const { core::mem::MaybeUninit::uninit() }; N]
    }

    pub unsafe fn initialize(arr: [core::mem::MaybeUninit<T>; N]) -> [T; N] {
        unsafe { core::mem::MaybeUninit::array_assume_init(arr) }
    }
}

impl<'a, T, const N: usize> Drop for ArrayGuard<'a, T, N> {
    fn drop(&mut self) {
        debug_assert!(self.initialized <= N, "invalid initialized state");
        if self.initialized == N {
            return;
        }

        for item in &mut self.arr[..self.initialized] {
            // SAFETY: only iterating over items that are already initialized
            unsafe { item.assume_init_drop() }
        }
    }
}

impl<const N: usize, I: Parse + Unpin> Parse for [I; N] {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let mut arr = ArrayGuard::<I, N>::uninit();
        {
            let mut guard = ArrayGuard::<I, N>::new(&mut arr);
            for _ in 0..N {
                let item = I::parse(reader, options)?;
                unsafe {
                    guard.push(item);
                }
            }
        }
        Ok(unsafe { ArrayGuard::initialize(arr) })
    }
}

pub struct ArrayParse<'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> {
    initialized: usize,
    storage: [core::mem::MaybeUninit<I>; N],
    state: AsyncIterState<'a, R, I::Fut<'a, R>>,
}

impl<'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> Drop
    for ArrayParse<'a, R, I, N>
{
    fn drop(&mut self) {
        debug_assert!(self.initialized <= N, "invalid initialized state");
        if self.initialized == N {
            return;
        }

        for item in &mut self.storage[..self.initialized] {
            // SAFETY: only iterating over items that are already initialized
            unsafe { item.assume_init_drop() }
        }
    }
}

impl<'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> Future
    for ArrayParse<'a, R, I, N>
{
    type Output = Result<[I; N], ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            match core::mem::replace(&mut self.state, AsyncIterState::Empty) {
                AsyncIterState::Iterating(mut fut) => {
                    match Pin::new(&mut fut).poll(cx) {
                        Poll::Pending => {
                            self.state = AsyncIterState::Iterating(fut);
                            return Poll::Pending;
                        }
                        Poll::Ready(i) => {
                            let i = i?;
                            debug_assert!(self.initialized < N, "trying to write past array");
                            let idx = self.initialized;
                            unsafe {
                                let uninit = self.storage.get_unchecked_mut(idx);
                                uninit.write(i);
                            }
                            self.initialized += 1;
                            if self.initialized == N {
                                let (reader, opts) = fut.take_reader();
                                self.state = AsyncIterState::Done(reader, opts);
                                let finished = core::mem::replace(
                                    &mut self.storage,
                                    [const { core::mem::MaybeUninit::uninit() }; N],
                                );
                                self.initialized = 0;
                                // SAFETY: all items are initialized
                                let arr = unsafe { ArrayGuard::initialize(finished) };
                                return Poll::Ready(Ok(arr));
                            }
                            let (reader, opts) = fut.take_reader();
                            self.state = AsyncIterState::Iterating(I::create_fut(reader, opts));
                            continue;
                        }
                    }
                }
                AsyncIterState::Done(r, o) if N == 0 => {
                    self.state = AsyncIterState::Done(r, o);
                    // SAFETY: array is empty
                    return Poll::Ready(Ok(unsafe {
                        ArrayGuard::initialize([const { core::mem::MaybeUninit::uninit() }; N])
                    }));
                }
                _ => panic!("AsyncIterState invalid state"),
            }
        }
    }
}

impl<'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> TakeReader<'a, R>
    for ArrayParse<'a, R, I, N>
{
    fn take_reader(mut self) -> (R, &'a ParseOptions) {
        let state = core::mem::replace(&mut self.state, AsyncIterState::Empty);
        state.take_reader()
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        self.state.borrow_reader()
    }
}

impl<const N: usize, I: AsyncParse + Unpin> AsyncParse for [I; N] {
    type Fut<'a, R: PollReader + Unpin> = ArrayParse<'a, R, I, N>;
    fn create_fut<'a, R: PollReader + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R> {
        let state = if N == 0 {
            AsyncIterState::Done(reader, options)
        } else {
            AsyncIterState::Iterating(I::create_fut(reader, options))
        };

        ArrayParse {
            initialized: 0,
            storage: [const { core::mem::MaybeUninit::uninit() }; N],
            state,
        }
    }
}
