#![feature(maybe_uninit_array_assume_init)]

macro_rules! impl_take_reader {
    () => {
        fn take_reader(self) -> (R, &'a ParseOptions) {
            match self {
                Self::Done(reader, opts) => return (reader, opts),
                _ => unreachable!("invalid state"),
            }
        }
    }
}

pub enum AsyncIterState<'a, R, T> {
    Iterating(T),
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R, T: TakeReader<'a, R>> TakeReader<'a, R> for AsyncIterState<'a, R, T> {
    impl_take_reader!{}
    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Iterating(t) => t.borrow_reader(),
            Self::Done(r, _) => r,
            Self::Empty => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AtomSize {
    #[cfg(not(feature = "extended_sized_atoms"))]
    size: u32,
    #[cfg(feature = "extended_sized_atoms")]
    size: u64,
}

impl AtomSize {
    pub fn until_eof(&self) -> bool {
        self.size == 0
    }

    const MIN_ATOM_SIZE_32: u32 = 8;
    #[cfg(feature = "extended_sized_atoms")]
    const MIN_ATOM_SIZE_64: u64 = 16;

    fn parse<T: Reader>(size: u32, reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        #[cfg(feature = "extended_sized_atoms")]
        {
            if !matches!(size, 0 | 1) && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            let size = if size == 1 {
                let extended_size = u64::parse(reader, options)?;
                if extended_size < Self::MIN_ATOM_SIZE_64 {
                    return Err(ParseError::AtomSizeTooSmall)
                }
                extended_size - Self::MIN_ATOM_SIZE_64
            } else {
                (size - Self::MIN_ATOM_SIZE_32) as u64
            };

            Ok(Self {
                size
            })
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            let _ = reader;
            let _ = options;
            if size == 1 {
                return Err(ParseError::AtomSizeUnsupported);
            }

            if size != 0 && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            Ok(Self {
                size: size - Self::MIN_ATOM_SIZE_32
            })
        }
    }

    fn create_fut<'a, R: PollReader + Unpin>(size: u32, reader: R, options: &'a ParseOptions) -> AtomSizeParse<'a, R> {
        #[cfg(feature = "extended_sized_atoms")]
        {
            if size == 1 {
                AtomSizeParse::ExtendedSize(u64::create_fut(reader, options))
            } else {
                AtomSizeParse::Waiting(size, reader, options)
            }
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            AtomSizeParse::Waiting(size, reader, options)
        }
    }
}

pub enum AtomSizeParse<'a, R: PollReader + Unpin> {
    Waiting(u32, R, &'a ParseOptions),
    #[cfg(feature = "extended_sized_atoms")]
    ExtendedSize(<u64 as AsyncParse>::Fut<'a, R>),
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin> Future for AtomSizeParse<'a, R> {
    type Output = Result<AtomSize, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        loop {
            let this = core::mem::replace(&mut *self, AtomSizeParse::Empty);
            match this {
                AtomSizeParse::Waiting(size, reader, opts) => {
                    *self = Self::Done(reader, opts);
                    #[cfg(not(feature = "extended_sized_atoms"))]
                    if size == 1 {
                        let _ = cx;
                        return core::task::Poll::Ready(Err(ParseError::AtomSizeUnsupported));
                    }

                    if size != 0 && size < AtomSize::MIN_ATOM_SIZE_32 {
                        return std::task::Poll::Ready(Err(ParseError::AtomSizeTooSmall))
                    }
                    return core::task::Poll::Ready(Ok(AtomSize {
                        #[cfg(feature = "extended_sized_atoms")]
                        size: u64::from(size),
                        #[cfg(not(feature = "extended_sized_atoms"))]
                        size,
                    }))
                }
                #[cfg(feature = "extended_sized_atoms")]
                AtomSizeParse::ExtendedSize(mut fut) => {
                    match core::pin::Pin::new(&mut fut).poll(cx) {
                        core::task::Poll::Pending => {
                            *self = AtomSizeParse::ExtendedSize(fut);
                            return core::task::Poll::Pending;
                        }
                        core::task::Poll::Ready(res) => {
                            let IntegerParse::Done(reader, options) = fut else {
                                unreachable!("invalid future state");
                            };
                            *self = AtomSizeParse::Done(reader, options);
                            return std::task::Poll::Ready(res.and_then(|extended_size| {
                                if extended_size < AtomSize::MIN_ATOM_SIZE_64 {
                                    Err(ParseError::AtomSizeTooSmall)
                                } else {
                                    Ok(AtomSize {
                                        size: extended_size - AtomSize::MIN_ATOM_SIZE_64
                                    })
                                }
                            }))
                        }
                    }
                }
                AtomSizeParse::Done(..) => panic!("polled future after completion"),
                AtomSizeParse::Empty => unreachable!(),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for AtomSizeParse<'a, R> {
    impl_take_reader!{}

    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Waiting(_, r, _) => r,
            #[cfg(feature = "extended_sized_atoms")]
            Self::ExtendedSize(s) => s.borrow_reader(),
            Self::Done(r, ..) => r,
            _ => unreachable!(),
        }
    }
}


type IoError = std::io::Error;

#[derive(thiserror::Error, Debug)]
#[error("could not convert between integers")]
pub struct TryFromIntError;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Atom Size is too small")]
    AtomSizeTooSmall,
    #[cfg(not(feature = "extended_sized_atoms"))]
    #[error("extended atom sizes (64 bytes) is not supported")]
    AtomSizeUnsupported,
    #[error("io error")]
    Io(#[from] IoError),
    #[error("could not convert between integers")]
    IntegerConversion(#[from] TryFromIntError)
}

macro_rules! parse_integers {
    ($($i:ty),*,) => {
        $(
            impl Parse for $i {
                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                    let mut buf = [0; core::mem::size_of::<$i>()];
                    reader.read(&mut buf)?;

                    Ok(if matches!(options.endianess, Endianess::Big) {
                        // likely path
                        <$i>::from_be_bytes(buf)
                    } else {
                        <$i>::from_le_bytes(buf)
                    })
                }
            }
            
            impl AsyncParse for $i {
                type Fut<'a, R: PollReader + Unpin> = IntegerParse<'a, R, $i, { core::mem::size_of::<$i>() }>;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    IntegerParse::Waiting(reader, options, [0; _], core::marker::PhantomData)
                }
            }

            impl <'a, R: PollReader + Unpin> Future for IntegerParse<'a, R, $i, { core::mem::size_of::<$i>() }> {
                type Output = Result<$i, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    let this = core::mem::replace(&mut *self, IntegerParse::Empty);
                    match this {
                        IntegerParse::Waiting(mut r, opts, mut buf, _) => {
                            match core::pin::Pin::new(&mut r).poll_read(cx, &mut buf) {
                                core::task::Poll::Pending => {
                                    *self = Self::Waiting(r, opts, buf, core::marker::PhantomData);
                                    return core::task::Poll::Pending
                                }
                                core::task::Poll::Ready(res) => {
                                    *self = Self::Done(r, opts);
                                    return std::task::Poll::Ready(Ok(res.map(|_| if matches!(opts.endianess, Endianess::Big) {
                                        // likely path
                                        <$i>::from_be_bytes(buf)
                                    } else {
                                        <$i>::from_le_bytes(buf)
                                    })?))
                                }
                            }
                        }
                        IntegerParse::Done(..) => panic!("future polled after completion"),
                        IntegerParse::Empty => unreachable!(),
                    }
                }
            }
        )*
    }
}

pub enum IntegerParse<'a, R, T, const N: usize> {
    Waiting(R, &'a ParseOptions, [u8; N], core::marker::PhantomData<T>),
    Done(R, &'a ParseOptions),
    Empty
}

impl <'a, R, T, const N: usize> TakeReader<'a, R> for IntegerParse<'a, R, T, N> {
    impl_take_reader!{}

    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Waiting(r, ..) => r,
            Self::Done(r, ..) => r,
            _ => unreachable!(),
        }
    }
}

parse_integers!{
    u8, u16, u32, u64,
    i8, i16, i32, i64,
}

pub trait Parse: Sized + AsyncParse {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError>;
    fn parse_async<'a, 'r, T: AsyncReader>(reader: &'r mut T, options: &'a ParseOptions) -> <Self as AsyncParse>::Fut<'a, &'r mut T> {
        <Self as AsyncParse>::create_fut(reader, options)
    }
}

pub trait TakeReader<'a, R> {
    fn take_reader(self) -> (R, &'a ParseOptions);
    fn borrow_reader(&mut self) -> &mut R;
}

pub trait AsyncParse: Sized {
    type Fut<'a, R: PollReader + Unpin>: Future<Output = Result<Self, ParseError>> + TakeReader<'a, R> + Unpin;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R>;
}

struct ArrayGuard<'a, T, const N: usize> {
    arr: &'a mut [core::mem::MaybeUninit<T>; N],
    initialized: usize,
}

impl <'a, T, const N: usize> ArrayGuard<'a, T, N> {
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
        [const { core::mem::MaybeUninit::uninit()}; N]
    }

    pub unsafe fn initialize(arr: [core::mem::MaybeUninit<T>; N]) -> [T; N] {
        unsafe { 
            core::mem::MaybeUninit::array_assume_init(arr)
        }
    }
}

impl <'a, T, const N: usize> Drop for ArrayGuard<'a, T, N> {
    fn drop(&mut self) {
        debug_assert!(self.initialized <= N, "invalid initialized state");
        if self.initialized == N {
            return;
        }

        for item in &mut self.arr[..self.initialized] {
            // SAFETY: only iterating over items that are already initialized
            unsafe {
                item.assume_init_drop()
            }
        }
    }
}

impl <const N: usize, I: Parse + Unpin> Parse for [I; N] {
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

impl <'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> Drop for ArrayParse<'a, R, I, N> {
    fn drop(&mut self) {
        debug_assert!(self.initialized <= N, "invalid initialized state");
        if self.initialized == N {
            return;
        }

        for item in &mut self.storage[..self.initialized] {
            // SAFETY: only iterating over items that are already initialized
            unsafe {
                item.assume_init_drop()
            }
        }
    }
}

impl <'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> Future for ArrayParse<'a, R, I, N> {
    type Output = Result<[I; N], ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
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
                                let finished = core::mem::replace(&mut self.storage, [const {core::mem::MaybeUninit::uninit()}; N]);
                                self.initialized = 0;
                                // SAFETY: all items are initialized
                                let arr = unsafe {
                                    ArrayGuard::initialize(finished)
                                };
                                return Poll::Ready(Ok(arr))
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
                        ArrayGuard::initialize([const {core::mem::MaybeUninit::uninit()}; N])
                    }))
                }
                _ => panic!("invalid state"),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin, I: AsyncParse + Unpin, const N: usize> TakeReader<'a, R> for ArrayParse<'a, R, I, N> {
    fn take_reader(mut self) -> (R, &'a ParseOptions) {
        let state = core::mem::replace(&mut self.state, AsyncIterState::Empty);
        state.take_reader()
    }
    fn borrow_reader(&mut self) -> &mut R {
        self.state.borrow_reader()
    }
}

impl <const N: usize, I: AsyncParse + Unpin> AsyncParse for [I; N] {
    type Fut<'a, R: PollReader + Unpin> = ArrayParse<'a, R, I, N>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        let state = if N == 0 {
            AsyncIterState::Done(reader, options)
        } else {
            AsyncIterState::Iterating(I::create_fut(reader, options))
        };

        ArrayParse {
            initialized: 0,
            storage: [const { core::mem::MaybeUninit::uninit()}; N],
            state,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct FourCC([u8; 4]);

impl core::fmt::Debug for FourCC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl Parse for FourCC {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let inner = u32::parse(reader, options)?;
        Ok(Self(inner.to_be_bytes()))
    }
}

pub struct FourCCParse<'a, R: PollReader + Unpin> {
    inner: <u32 as AsyncParse>::Fut<'a, R>,
}

impl <'a, R: PollReader + Unpin> Future for FourCCParse<'a, R> {
    type Output = Result<FourCC, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let mut this = core::mem::replace(&mut *self, FourCCParse { inner: IntegerParse::Empty });
        use core::task::Poll;
        match core::pin::Pin::new(&mut this.inner).poll(cx) {
            Poll::Pending => {
                core::mem::swap(&mut this, &mut *self);
                return Poll::Pending;
            }
            Poll::Ready(res) => {
                core::mem::swap(&mut this, &mut *self);
                return Poll::Ready(res.map(|num| FourCC(num.to_be_bytes())))
            }
        }
    }
}

impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for FourCCParse<'a, R> {
    fn take_reader(self) -> (R, &'a ParseOptions) { 
        self.inner.take_reader()
    }
    fn borrow_reader(&mut self) -> &mut R {
        self.inner.borrow_reader()
    }
}

impl AsyncParse for FourCC {
    type Fut<'a, R: PollReader + Unpin> = FourCCParse<'a, R>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        FourCCParse {
            inner: u32::create_fut(reader, options),
        }
        
    }
}

#[derive(Debug)]
pub struct AtomHeader {
    pub size: AtomSize,
    pub fcc: FourCC,
}

impl Parse for AtomHeader {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = u32::parse(reader, options)?;
        let fcc = FourCC::parse(reader, options)?;
        let size = AtomSize::parse(size, reader, options)?;
        Ok(Self {
            size,
            fcc,
        })
    }
}

pub enum AtomHeaderParse<'a, R: PollReader + Unpin> {
    Waiting(<u32 as AsyncParse>::Fut<'a, R>),
    Fourcc {
        size: u32,
        fourcc: <FourCC as AsyncParse>::Fut<'a, R>,
    },
    AtomSize {
        fourcc: FourCC,
        // the u32 size from before is moved here
        size: AtomSizeParse<'a, R>,
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin> Future for AtomHeaderParse<'a, R> {
    type Output = Result<AtomHeader, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Waiting(mut fut) => {
                    match Pin::new(&mut fut).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Waiting(fut);
                            return Poll::Pending;
                        }
                        Poll::Ready(size) => {
                            let size = size?;
                            let IntegerParse::Done(reader, options) = fut else {
                                unreachable!("bad future state");
                            };
                            *self = AtomHeaderParse::Fourcc {
                                size,
                                fourcc: FourCC::create_fut(reader, options),
                            }
                        }
                    }
                }
                Self::Fourcc {
                    size,
                    mut fourcc,
                } => {
                    match Pin::new(&mut fourcc).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Fourcc { size, fourcc };
                            return Poll::Pending;
                        }
                        Poll::Ready(fcc) => {
                            let fcc = fcc?;
                            let IntegerParse::Done(reader, opts) = fourcc.inner else {
                                unreachable!("invalid state");
                            };
                            *self = Self::AtomSize{
                                fourcc: fcc, 
                                size: AtomSize::create_fut(size, reader, opts)
                            };
                        }
                    }
                }
                Self::AtomSize {
                    fourcc,
                    mut size,
                } => {
                    match Pin::new(&mut size).poll(cx) {
                        Poll::Pending => {
                            *self = Self::AtomSize { fourcc, size };
                            return Poll::Pending;
                        }
                        Poll::Ready(atom_size) => {
                            let AtomSizeParse::Done(reader, opts) = size else {
                                unreachable!("invalid state");
                            };
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(atom_size.map(|size| {
                                AtomHeader {
                                    size,
                                    fcc: fourcc,
                                }
                            }))
                        }
                    }
                }
                Self::Done(..) => panic!("repoll future"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for AtomHeaderParse<'a, R> {
    impl_take_reader!{}

    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Waiting(r) => r.borrow_reader(),
            Self::Fourcc { fourcc, .. } => fourcc.borrow_reader(),
            Self::AtomSize { size, ..} => size.borrow_reader(),
            Self::Done(r, ..) => r,
            _ => unreachable!(),
        }
    }
}

impl AsyncParse for AtomHeader {
    type Fut<'a, R: PollReader + Unpin> = AtomHeaderParse<'a, R>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        AtomHeaderParse::Waiting(u32::create_fut(reader, options))
    }
}

pub trait Atom: Sized {
    const FCC: FourCC;
    // Ok(exact) if known staticly, Err((lower, upper)) if known size range
    // this is the size *not* including the atoms header (e.g 4 + 4 u32, 4 + 4 + 8 u64)
    /*
    const BODY_SIZE: Result<usize, (usize, usize)>;
    fn parse_body() -> Result<Self, ParseError>;
    */
}

pub trait SwapOffsets {
    fn swap_offsets(&mut self, offset: &mut usize);
}

pub trait Reader: SwapOffsets {
    fn remaining_size(&self) -> usize;
    fn offset(&self) -> usize;
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError>;
    // reads a cstr and returns its length (not including nul) 
    // and advances the cursor after the string
    fn read_cstr(&mut self) -> Result<usize, IoError>;
    fn seek(&mut self, amt: usize) -> Result<(), IoError>;

    fn seek_remaining(&mut self) -> Result<(), IoError> {
        self.seek(self.remaining_size())
    }
}

impl <'a, S: SwapOffsets> SwapOffsets for &'a mut S {
    fn swap_offsets(&mut self, offset: &mut usize) {
        S::swap_offsets(self, offset)
    }
}

impl <'a, R: Reader> Reader for &'a mut R {
    fn remaining_size(&self) -> usize {
        R::remaining_size(self)
    }

    fn offset(&self) -> usize {
        R::offset(self)
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        R::read(self, bytes)
    }

    fn read_cstr(&mut self) -> Result<usize, IoError> {
        R::read_cstr(self)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        R::seek(self, amt)
    }
}

mod async_impl {
    use super::IoError;
    use core::pin::Pin;
    use core::task::{Poll, Context, ready};
    pub trait PollReader {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>>;

        fn poll_read_cstr(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<usize, IoError>>;

        fn seek_start(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: usize,
        ) -> Result<(), IoError>;

        fn poll_seek_complete(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>>;

        fn offset(&self) -> usize;
        fn remaining_size(&self) -> usize;
    }

    impl <'a, R: ?Sized + Unpin + PollReader> PollReader for &'a mut R {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut **self).poll_read(cx, buf)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<usize, IoError>> {
            Pin::new(&mut **self).poll_read_cstr(cx)
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            Pin::new(&mut **self).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut **self).poll_seek_complete(cx)
        }

        fn offset(&self) -> usize {
            R::offset(self)
        }

        fn remaining_size(&self) -> usize {
            R::remaining_size(self)
        }
    }

    use pin_project::pin_project;
    use core::marker::PhantomPinned;
    use core::future::Future;
    #[pin_project]
    pub struct AsyncRead<'a, R: ?Sized> {
        reader: &'a mut R,
        buf: &'a mut [u8],
        #[pin]
        _pin: PhantomPinned,
    }

    pub(crate) fn read<'a, R: ?Sized + PollReader>(reader: &'a mut R, buf: &'a mut [u8]) -> AsyncRead<'a, R> {
        AsyncRead {
            reader,
            buf,
            _pin: PhantomPinned,
        }
    }

    impl <R: ?Sized + PollReader + Unpin> Future for AsyncRead<'_, R> {
        type Output = Result<(), IoError>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
            let me = self.project();
            ready!(Pin::new(me.reader).poll_read(cx, me.buf))?;
            Poll::Ready(Ok(()))
        }
    }

    #[pin_project(UnsafeUnpin)]
    pub struct AsyncReadCstr<R> {
        pub reader: R,
        #[pin]
        _pin: PhantomPinned,
    }

    unsafe impl <R: Unpin> pin_project::UnsafeUnpin for AsyncReadCstr<R> {}

    pub(crate) fn read_cstr<'a, R: PollReader>(reader: R) -> AsyncReadCstr<R> {
        AsyncReadCstr {
            reader,
            _pin: PhantomPinned,
        }
    }

    impl <R: PollReader + Unpin> Future for AsyncReadCstr<R> {
        type Output = Result<usize, IoError>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<usize, IoError>> {
            let me = self.project();
            let len = ready!(Pin::new(me.reader).poll_read_cstr(cx))?;
            Poll::Ready(Ok(len))
        }
    }

    #[pin_project(UnsafeUnpin)]
    pub struct AsyncSeek<R> {
        pub reader: R,
        amt: Option<usize>,
        #[pin]
        _pin: PhantomPinned,
    }

    unsafe impl <R: Unpin> pin_project::UnsafeUnpin for AsyncSeek<R> {}

    pub(crate) fn seek<R: PollReader>(reader: R, amt: usize) -> AsyncSeek<R> {
        AsyncSeek {
            reader,
            amt: Some(amt),
            _pin: PhantomPinned,
        }
    }

    impl <R: PollReader + Unpin> Future for AsyncSeek<R> {
        type Output = Result<(), IoError>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
            let me = self.project();
            match me.amt {
                Some(amt) => {
                    ready!(Pin::new(&mut *me.reader).poll_seek_complete(cx))?;
                    match Pin::new(&mut *me.reader).seek_start(cx, *amt) {
                        Ok(()) => {
                            *me.amt = None;
                            Pin::new(&mut *me.reader).poll_seek_complete(cx)
                        }
                        Err(e) => Poll::Ready(Err(e))
                    }
                }
                None => {
                    Pin::new(&mut *me.reader).poll_seek_complete(cx)
                }
            }
        }
    }

    use super::{BacktrackReader, TrailingReader, SwapOffsets};

    impl <R: PollReader + Unpin> PollReader for TrailingReader<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            if buf.len() > self.remaining_size() {
                return Poll::Ready(Err(std::io::Error::other("not enough spc")));
            }
            Pin::new(&mut self.reader).poll_read(cx, buf)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<usize, IoError>> {
            let len = ready!(Pin::new(&mut self.reader).poll_read_cstr(cx))?;

            if len > self.remaining_size() {
                return Poll::Ready(Err(std::io::Error::other("not enough spc")));
            }
            Poll::Ready(Ok(len))
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete(cx)
        }

        fn offset(&self) -> usize {
            self.reader.offset()
        }

        fn remaining_size(&self) -> usize {
            self.max_offset.checked_sub(self.reader.offset()).unwrap_or_default()
        }
    }

    impl <R: PollReader + SwapOffsets + Unpin> PollReader for BacktrackReader<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_read(cx, buf)
        }

        fn poll_read_cstr(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<usize, IoError>> {
            Pin::new(&mut self.reader).poll_read_cstr(cx)
        }

        fn seek_start(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            amt: usize,
        ) -> Result<(), IoError> {
            Pin::new(&mut self.reader).seek_start(cx, amt)
        }

        fn poll_seek_complete(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), IoError>> {
            Pin::new(&mut self.reader).poll_seek_complete(cx)
        }

        fn offset(&self) -> usize {
            self.reader.offset()
        }

        fn remaining_size(&self) -> usize {
            self.reader.remaining_size()
        }
    }
}

use async_impl::{AsyncRead, AsyncReadCstr, AsyncSeek, PollReader};

pub trait AsyncReader: SwapOffsets + PollReader + Unpin {
    fn async_read<'a>(&'a mut self, bytes: &'a mut [u8]) -> AsyncRead<'a, Self> {
        async_impl::read(self, bytes)
    }
    fn async_seek(&mut self, amt: usize) -> AsyncSeek<&mut Self> {
        async_impl::seek(self, amt)
    }

    // reads a cstr and returns its length (including nul)
    fn async_read_cstr(&mut self) -> AsyncReadCstr<&mut Self> {
        async_impl::read_cstr(self)
    }

    fn seek_remaining(&mut self) -> AsyncSeek<&mut Self> {
        self.async_seek(self.remaining_size())
    }
}

impl <'r, R: AsyncReader> AsyncReader for &'r mut R {
    fn async_read<'a>(&'a mut self, bytes: &'a mut [u8]) -> AsyncRead<'a, Self> {
        async_impl::read(self, bytes)
    }
    fn async_seek(&mut self, amt: usize) -> AsyncSeek<&mut Self> {
        async_impl::seek(self, amt)
    }

    // reads a cstr and returns its length (including nul)
    fn async_read_cstr(&mut self) -> AsyncReadCstr<&mut Self> {
        async_impl::read_cstr(self)
    }

    fn seek_remaining(&mut self) -> AsyncSeek<&mut Self> {
        self.async_seek(self.remaining_size())
    }
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum Endianess {
    #[default]
    Big,
    Little,
}

#[derive(Default)]
pub struct ParseOptions {
    endianess: Endianess,
}

pub struct InMemoryReader {
    bytes: Vec<u8>,
    offset: usize,
}

impl InMemoryReader {
    pub fn from_path<P: AsRef<std::path::Path>>(p: P) -> Result<Self, IoError> {
        let bytes = std::fs::read(p)?;
        Ok(Self {
            bytes,
            offset: 0,
        })
    }
}

impl SwapOffsets for InMemoryReader {
    fn swap_offsets(&mut self, offset: &mut usize) {
        core::mem::swap(&mut self.offset, offset)
    }
}

impl Reader for InMemoryReader {
    fn remaining_size(&self) -> usize {
        self.bytes.len().checked_sub(self.offset).unwrap_or_default()
    }

    fn offset(&self) -> usize {
        core::cmp::min(self.offset, self.bytes.len())
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        let remaining = Reader::remaining_size(self);
        if remaining < bytes.len() {
            return Err(IoError::other("not enough spc"));
        }
        bytes.copy_from_slice(&self.bytes[self.offset..self.offset+bytes.len()]);
        self.offset += bytes.len();
        Ok(())
    }

    fn read_cstr(&mut self) -> Result<usize, IoError> {
        let bytes = &self.bytes[self.offset..];
        let len = bytes
            .iter()
            .position(|&byte| byte == b'\0').ok_or(IoError::other("not enough spc"))?;
        self.offset += len + 1;
        Ok(len)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        let remaining = Reader::remaining_size(self);
        if amt > remaining {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.offset += amt;
        Ok(())
    }
}

impl PollReader for InMemoryReader {
    fn poll_read(
        mut self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> core::task::Poll<Result<(), IoError>> {
        let remaining = PollReader::remaining_size(&*self);
        if remaining < buf.len() {
            return core::task::Poll::Ready(Err(IoError::other("not enough spc")));
        }

        buf.copy_from_slice(&self.bytes[self.offset..self.offset+buf.len()]);
        self.offset += buf.len();

        core::task::Poll::Ready(Ok(()))
    }

    fn poll_read_cstr(
        mut self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<usize, IoError>> {
        let bytes = &self.bytes[self.offset..];
        let len = bytes
            .iter()
            .position(|&byte| byte == b'\0').ok_or(IoError::other("not enough spc"))?;
        self.offset += len + 1;
        core::task::Poll::Ready(Ok(len))
    }

    fn seek_start(
        mut self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
        amt: usize,
    ) -> Result<(), IoError> {
        let remaining = PollReader::remaining_size(&*self);
        if amt > remaining {
            return Err(std::io::Error::other("not enough spc"));
        }

        self.offset += amt;
        Ok(())
    }

    fn poll_seek_complete(
        self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), IoError>> {
        core::task::Poll::Ready(Ok(()))
    }

    fn offset(&self) -> usize {
        core::cmp::min(self.offset, self.bytes.len())
    }
    fn remaining_size(&self) -> usize {
        self.bytes.len().checked_sub(self.offset).unwrap_or_default()
    }
}

pub struct ChildrenIter<'a, R: SwapOffsets, C> {
    _pd: core::marker::PhantomData<C>,
    pub reader: BacktrackReader<TrailingReader<&'a mut R>>,
    opts: &'a ParseOptions,
}

impl <'a, R: SwapOffsets + Reader, C> ChildrenIter<'a, R, C> {
    pub fn from_children(children: &Children<C>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let Children {
            offset,
            size,
            ..
        } = children;

        let max_offset = offset + size;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset);
        Self {
            reader,
            opts,
            _pd: core::marker::PhantomData,
        }
    }
}

use futures_core::Stream as AsyncIterator;

pub struct AsyncChildrenIter<'a, R: SwapOffsets + PollReader + Unpin, C: AsyncParse> {
    state: AsyncIterState<'a, BacktrackReader<TrailingReader<&'a mut R>>, <C as AsyncParse>::Fut<'a, BacktrackReader<TrailingReader<&'a mut R>>>>
}

impl <'a, R: SwapOffsets + PollReader + Unpin + Reader, C: AsyncParse> AsyncChildrenIter<'a, R, C> {
    pub fn from_children(children: &Children<C>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let Children {
            offset,
            size,
            ..
        } = children;


        let max_offset = offset + size;
        let reader = BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset);
        let remaining = PollReader::remaining_size(&reader);

        let state = if remaining == 0 {
            AsyncIterState::Done(reader, opts)
        } else {
            AsyncIterState::Iterating(C::create_fut(reader, opts))
        };

        Self {
            state
        }
    }
}

impl <'a, R: Reader, C: Parse> Iterator for ChildrenIter<'a, R, C> {
    type Item = Result<C, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining_size() > 0 {
            Some(C::parse(&mut self.reader, self.opts))
        } else {
            None
        }
    }
}

impl <'a, R: SwapOffsets + PollReader + Unpin, C: AsyncParse + Unpin> AsyncIterator for AsyncChildrenIter<'a, R, C> where <C as AsyncParse>::Fut<'a, R>: Unpin {
    type Item = Result<C, ParseError>;
    fn poll_next(mut self: core::pin::Pin<&mut Self>, ctx: &mut core::task::Context<'_>) -> core::task::Poll<Option<Self::Item>> {
        use core::pin::Pin;
        use core::task::Poll;

        let state = core::mem::replace(&mut *self, Self { state: AsyncIterState::Empty }).state;

        match state {
            AsyncIterState::Done(r, o) => {
                self.state = AsyncIterState::Done(r, o);
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
                            AsyncIterState::Iterating(C::create_fut(reader, opts))
                        };
                        return Poll::Ready(Some(res))
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

impl <'a, R: SwapOffsets + PollReader + Unpin, C: AsyncParse + Unpin> TakeReader<'a, BacktrackReader<TrailingReader<&'a mut R>>> for AsyncChildrenIter<'a, R, C> where <C as AsyncParse>::Fut<'a, R>: Unpin {
    fn take_reader(self) -> (BacktrackReader<TrailingReader<&'a mut R>>, &'a ParseOptions) {
        self.state.take_reader()
    }
    fn borrow_reader(&mut self) -> &mut BacktrackReader<TrailingReader<&'a mut R>> {
        self.state.borrow_reader()
    }
}

#[derive(Debug)]
pub struct Children<T> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: usize,
}

#[derive(Debug)]
pub struct SizedChildren<S, T> {
    _pd: core::marker::PhantomData<T>,
    len: S,
    offset: usize,
}

#[derive(Debug)]
pub struct PascalString<S> {
    len: S,
    offset: usize,
}

pub struct Extent<O, S> {
    pub offset: O,
    pub len: S,
}

impl <S: Copy> PascalString<S> {
    pub fn extent(&self) -> Extent<usize, S> {
        let Self {
            len,
            offset,
        } = self;

        Extent {
            offset: *offset,
            len: *len,
        }
    }
}

impl <S: Parse + TryInto<usize> + Copy + Unpin> Parse for PascalString<S> {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let len = S::parse(reader, options)?;
        let offset = reader.offset();
        let length: usize = len.try_into().map_err(|_| TryFromIntError)?;

        reader.seek(length)?;
        Ok(Self {
            len,
            offset,
        })
    }
}

pub enum PascalStringParse<'a, R: PollReader + Unpin, S: AsyncParse + TryInto<usize> + Unpin> where <S as AsyncParse>::Fut<'a, R>: Unpin {
    Waiting(<S as AsyncParse>::Fut<'a, R>),
    Seeking{
        offset: usize, 
        len: S, 
        fut: AsyncSeek<R>, 
        opts: &'a ParseOptions
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin, S: AsyncParse + TryInto<usize> + Unpin + Copy> Future for PascalStringParse<'a, R, S> where <S as AsyncParse>::Fut<'a, R>: Unpin {
    type Output = Result<PascalString<S>, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            match core::mem::replace(&mut *self, Self::Empty) {
                Self::Waiting(mut s) => {
                    match Pin::new(&mut s).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Waiting(s);
                            return Poll::Pending;
                        }
                        Poll::Ready(res) => {
                            let res = res?;
                            let (reader, opts) = s.take_reader();
                            let len = res.try_into().map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;
                            let offset = reader.offset();

                            *self = Self::Seeking {
                                offset,
                                len: res,
                                fut: async_impl::seek(reader, len),
                                opts,
                            }
                        }
                    }
                }
                Self::Seeking {
                    offset,
                    len,
                    mut fut,
                    opts,
                } => {
                    match Pin::new(&mut fut).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Seeking{
                                offset,
                                len, 
                                fut, 
                                opts
                            };
                            return Poll::Pending;
                        }
                        Poll::Ready(res) => {
                            let reader = fut.reader;
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(Ok(res.map(|_| PascalString {
                                offset,
                                len,
                            })?))
                        }
                    }
                }
                Self::Done(..) => panic!("polled after done"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin, S: AsyncParse + TryInto<usize> + Unpin> TakeReader<'a, R> for PascalStringParse<'a, R, S> where <S as AsyncParse>::Fut<'a, R>: Unpin {
    impl_take_reader!{}
    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Waiting(w) => w.borrow_reader(),
            Self::Seeking {
                fut,
                ..
            } => &mut fut.reader,
            Self::Done(r, ..) => r,
            _ => unreachable!(),
        }
    }
}

impl <S: AsyncParse + TryInto<usize> + Unpin + Copy> AsyncParse for PascalString<S> {
    type Fut<'a, R: PollReader + Unpin> = PascalStringParse<'a, R, S>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        PascalStringParse::Waiting(S::create_fut(reader, options))
    }
}

pub struct NullTerminatedString {
    len: usize,
    offset: usize,
}

impl NullTerminatedString {
    pub fn extent(&self) -> Extent<usize, usize> {
        let Self {
            len,
            offset,
        } = self;

        Extent {
            len: *len,
            offset: *offset,
        }
    }
}

impl Parse for NullTerminatedString {
    fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let len = reader.read_cstr()?;

        Ok(Self {
            offset,
            len,
        })
    }
}

pub struct NullTerminatedStringParse<'a, R>(usize, AsyncReadCstr<R>, &'a ParseOptions);

impl <'a, R: PollReader + Unpin> Future for NullTerminatedStringParse<'a, R> {
    type Output = Result<NullTerminatedString, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        use core::task::{Poll, ready};
        use core::pin::Pin;
        let len = ready!(Pin::new(&mut self.1).poll(cx))?;
        Poll::Ready(Ok(NullTerminatedString {
            offset: self.0,
            len,
        }))
    }
}

impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for NullTerminatedStringParse<'a, R> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.1.reader, self.2)
    }
    fn borrow_reader(&mut self) -> &mut R {
        &mut self.1.reader
    }
}

impl AsyncParse for NullTerminatedString {
    type Fut<'a, R: PollReader + Unpin> = NullTerminatedStringParse<'a, R>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        let offset = reader.offset();
        NullTerminatedStringParse(offset, async_impl::read_cstr(reader), options)
    }
}

impl <S: Parse + Unpin, I: Parse + Unpin> Parse for SizedChildren<S, I> {
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

pub enum SizedChildrenParse<'a, R: PollReader + Unpin, S: AsyncParse + Unpin, C> where <S as AsyncParse>::Fut<'a, R>: Unpin {
    Size {
        offset: usize,
        fut: <S as AsyncParse>::Fut<'a, R>, 
        _pd: core::marker::PhantomData<C>
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin, S: AsyncParse + Unpin, C: AsyncParse + Unpin> Future for SizedChildrenParse<'a, R, S, C> where <S as AsyncParse>::Fut<'a, R>: Unpin {
    type Output = Result<SizedChildren<S, C>, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        let this = core::mem::replace(&mut *self, Self::Empty);
        match this {
            Self::Size {
                offset,
                mut fut, 
                ..
            } => {
                match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Size{fut, offset, _pd: core::marker::PhantomData};
                        return Poll::Pending;
                    }
                    Poll::Ready(size) => {
                        let (reader, options) = fut.take_reader();
                        *self = Self::Done(reader, options);
                        return Poll::Ready(size.map(|len| {
                            SizedChildren {
                                offset,
                                len,
                                _pd: core::marker::PhantomData,
                            }
                        }))
                    }
                }
            }
            Self::Done(..) => panic!("poll after completion"),
            Self::Empty => unreachable!(),
        }
    }
}

impl <'a, R: PollReader + Unpin, S: AsyncParse + Unpin, C: AsyncParse> TakeReader<'a, R> for SizedChildrenParse<'a, R, S, C> {
    impl_take_reader!{}
    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Size {
                fut,
                ..
            } => fut.borrow_reader(),
            Self::Done(r, _) => r,
            Self::Empty => unreachable!(),
        }
    }
}

impl <S: AsyncParse + Unpin, C: AsyncParse + Unpin> AsyncParse for SizedChildren<S, C> {
    type Fut<'a, R: PollReader + Unpin> = SizedChildrenParse<'a, R, S, C>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        let offset = reader.offset();
        SizedChildrenParse::Size { offset, fut: S::create_fut(reader, options), _pd: core::marker::PhantomData }
    }
}

impl <I: Parse + Unpin> Parse for Children<I> {
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

pub struct ChildrenParse<'a, R, C>(R, &'a ParseOptions, core::marker::PhantomData<C>);
impl <'a, R: PollReader + Unpin, C: AsyncParse> Future for ChildrenParse<'a, R, C> {
    type Output = Result<Children<C>, ParseError>;
    fn poll(self: core::pin::Pin<&mut Self>, _: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.0.remaining_size();
        core::task::Poll::Ready(Ok(Children {
            offset,
            size,
            _pd: core::marker::PhantomData,
        }))
    }
}

impl <'a, R: PollReader + Unpin, C: AsyncParse> TakeReader<'a, R> for ChildrenParse<'a, R, C> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> &mut R {
        &mut self.0
    }
}

impl <C: AsyncParse + Unpin> AsyncParse for Children<C> {
    type Fut<'a, R: PollReader + Unpin> = ChildrenParse<'a, R, C>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        ChildrenParse(reader, options, core::marker::PhantomData)
    }
}

pub struct BacktrackReader<R: SwapOffsets> {
    reader: R,
    stored_offset: usize,
}

impl <R: SwapOffsets> BacktrackReader<R> {
    pub fn new(mut reader: R, mut backtrack_to: usize) -> Self {
        reader.swap_offsets(&mut backtrack_to);
        Self {
            reader,
            stored_offset: backtrack_to,
        }
    }
}

impl <R: SwapOffsets> Drop for BacktrackReader<R> {
    fn drop(&mut self) {
        self.reader.swap_offsets(&mut self.stored_offset)
    }
}

impl <S: SwapOffsets> SwapOffsets for BacktrackReader<S> {
    fn swap_offsets(&mut self, offset: &mut usize) {
        self.reader.swap_offsets(offset)
    }
}

impl <R: Reader> Reader for BacktrackReader<R> {
    fn remaining_size(&self) -> usize {
        self.reader.remaining_size()
    }
    fn offset(&self) -> usize {
        self.reader.offset()
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        self.reader.read(bytes)
    }

    fn read_cstr(&mut self) -> Result<usize, IoError> {
        self.reader.read_cstr()
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        self.reader.seek(amt)
    }
}

pub struct TrailingReader<R> {
    reader: R,
    max_offset: usize,
}

impl <R: Reader> TrailingReader<R> {
    pub fn new(reader: R, max_offset: usize) -> Self {
        Self {
            reader,
            max_offset,
        }
    }
}

impl <S: SwapOffsets> SwapOffsets for TrailingReader<S> {
    fn swap_offsets(&mut self, offset: &mut usize) {
        self.reader.swap_offsets(offset)
    }
}

impl <R: Reader> Reader for TrailingReader<R> {
    fn remaining_size(&self) -> usize {
        self.max_offset.checked_sub(self.reader.offset()).unwrap_or_default()
    }

    fn offset(&self) -> usize {
        self.reader.offset()
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        if bytes.len() > self.remaining_size() {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.reader.read(bytes)?;
        Ok(())
    }

    fn read_cstr(&mut self) -> Result<usize, IoError> {
        let len = self.reader.read_cstr()?;
        if len > self.remaining_size() {
            return Err(std::io::Error::other("not enough spc"));
        }
        Ok(len)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        if amt > self.remaining_size() {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.reader.seek(amt)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Trailing<T> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: usize,
}

#[derive(Debug)]
pub struct Payload {
    offset: usize,
    size: usize,
}

impl Payload {
    pub fn extent(&self) -> Extent<usize, usize> {
        let Self {
            offset,
            size,
        } = self;

        Extent {
            offset: *offset,
            len: *size,
        }
    }
}

impl Parse for Payload {
    fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
        })
    }
}

pub struct PayloadParse<'a, R>(R, &'a ParseOptions);
impl <'a, R: PollReader + Unpin> Future for PayloadParse<'a, R> {
    type Output = Result<Payload, ParseError>;
    fn poll(self: core::pin::Pin<&mut Self>, _: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.0.remaining_size();
        core::task::Poll::Ready(Ok(Payload {
            offset,
            size,
        }))
    }
}

impl <'a, R> TakeReader<'a, R> for PayloadParse<'a, R> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> &mut R {
        &mut self.0
    }
}

impl AsyncParse for Payload {
    type Fut<'a, R: PollReader + Unpin> = PayloadParse<'a, R>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        PayloadParse(reader, options)
    }
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
    fn borrow_reader(&mut self) -> &mut R {
        &mut self.0
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
    fn borrow_reader(&mut self) -> &mut BacktrackReader<TrailingReader<&'a mut R>> {
        self.state.borrow_reader()
    }
}

#[derive(Debug)]
pub struct DynamicArray<S, I, const ZERO_RELATIVE: bool> {
    size: S,
    offset: usize,
    _pd: core::marker::PhantomData<I>,
}

pub struct DynamicArrayIter<'a, R: SwapOffsets, S, I, const ZERO_RELATIVE: bool> {
    size: S,
    current: S,
    exhausted: bool,
    pub reader: BacktrackReader<&'a mut R>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<I>,
}

impl <'a, R: SwapOffsets, S: ArraySize + Copy, I, const ZERO_RELATIVE: bool> DynamicArrayIter<'a, R, S, I, ZERO_RELATIVE> {
    pub fn from_dynamic_array(arr: &DynamicArray<S, I, ZERO_RELATIVE>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let DynamicArray {
            offset,
            size,
            ..
        } = arr;
        Self::new(*size, reader, *offset, opts)
    }

    pub fn from_sized_chidlren(children: &SizedChildren<S, I>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let SizedChildren {
            len,
            offset,
            ..
        } = children;
        Self::new(*len, reader, *offset, opts)
    }

    fn new(size: S, reader: &'a mut R, offset: usize, opts: &'a ParseOptions) -> Self {
        Self {
            size,
            current: S::ZERO,
            exhausted: false,
            reader: BacktrackReader::new(reader, offset),
            opts,
            _pd: core::marker::PhantomData,
        }
    }
}

fn dynamic_array_iter_has_next<const ZERO_RELATIVE: bool, S: ArraySize>(current: &mut S, max_size: &S, exhausted: &mut bool) -> bool {
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

impl <'a, R: Reader, S: ArraySize, I: Parse, const ZERO_RELATIVE: bool> Iterator for DynamicArrayIter<'a, R, S, I, ZERO_RELATIVE> {
    type Item = Result<I, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        let has_next = dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut self.current, &self.size, &mut self.exhausted);

        if !has_next {
            return None;
        }
        Some(I::parse(&mut self.reader, &self.opts))
    }
}

pub struct AsyncDynamicArrayIter<'a, R: SwapOffsets + PollReader + Unpin, S, I: AsyncParse, const ZERO_RELATIVE: bool> {
    size: S,
    current: S,
    exhausted: bool,

    state: AsyncIterState<'a, BacktrackReader<&'a mut R>, I::Fut<'a, BacktrackReader<&'a mut R>>>,
}

impl <'a, R: SwapOffsets + PollReader + Unpin, S: ArraySize + Copy, I: AsyncParse, const ZERO_RELATIVE: bool> AsyncDynamicArrayIter<'a, R, S, I, ZERO_RELATIVE> {
    pub fn from_dynamic_array(arr: &DynamicArray<S, I, ZERO_RELATIVE>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let DynamicArray {
            offset,
            size,
            ..
        } = arr;
        Self::new(*size, reader, *offset, opts)
    }

    pub fn from_sized_chidlren(children: &SizedChildren<S, I>, reader: &'a mut R, opts: &'a ParseOptions) -> Self {
        let SizedChildren {
            len,
            offset,
            ..
        } = children;
        Self::new(*len, reader, *offset, opts)
    }

    fn new(size: S, reader: &'a mut R, offset: usize, opts: &'a ParseOptions) -> Self {
        let reader = BacktrackReader::new(reader, offset);

        let mut exhausted = false;
        let mut current = S::ZERO;

        let state = if dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut current, &size, &mut exhausted) {
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
}

impl <'a, R: SwapOffsets + PollReader + Unpin, S: ArraySize + Unpin, I: AsyncParse + Unpin, const ZERO_RELATIVE: bool> AsyncIterator for AsyncDynamicArrayIter<'a, R, S, I, ZERO_RELATIVE> where <I as AsyncParse>::Fut<'a, R>: Unpin {
    type Item = Result<I, ParseError>;
    fn poll_next(mut self: core::pin::Pin<&mut Self>, ctx: &mut core::task::Context<'_>) -> core::task::Poll<Option<Self::Item>> {
        use core::pin::Pin;
        use core::task::Poll;
        
        let mut this = core::mem::replace(&mut *self, Self { size: S::ZERO, current: S::ZERO, exhausted: false, state: AsyncIterState::Empty });
        match this.state {
            AsyncIterState::Done(r, o) => {
                this.state = AsyncIterState::Done(r, o);
                core::mem::swap(&mut *self, &mut this);
                return Poll::Ready(None)
            }
            AsyncIterState::Iterating(mut i) => {
                match Pin::new(&mut i).poll(ctx) {
                    Poll::Pending => {
                        this.state = AsyncIterState::Iterating(i);
                        core::mem::swap(&mut *self, &mut this);
                        return Poll::Pending;
                    }
                    Poll::Ready(res) => {
                        let (reader, opts) = i.take_reader();
                        this.state = if dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut this.current, &this.size, &mut this.exhausted) {
                            AsyncIterState::Iterating(I::create_fut(reader, opts))
                        } else {
                            AsyncIterState::Done(reader, opts)
                        };
                        core::mem::swap(&mut *self, &mut this);
                        return Poll::Ready(Some(res))
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

impl <'a, R: SwapOffsets + PollReader + Unpin, S: ArraySize + Unpin, I: AsyncParse + Unpin, const ZERO_RELATIVE: bool> TakeReader<'a, BacktrackReader<&'a mut R>> for AsyncDynamicArrayIter<'a, R, S, I, ZERO_RELATIVE> where <I as AsyncParse>::Fut<'a, R>: Unpin {
    fn take_reader(self) -> (BacktrackReader<&'a mut R>, &'a ParseOptions) {
        self.state.take_reader()
    }
    fn borrow_reader(&mut self) -> &mut BacktrackReader<&'a mut R> {
        self.state.borrow_reader()
    }
}

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
    }
}

impl_array_size!{
    u16, u32,
}

impl <I: Parse + Unpin, S: Parse + ArraySize + Unpin, const ZERO_RELATIVE: bool> Parse for DynamicArray<S, I, ZERO_RELATIVE> {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = S::parse(reader, options)?;
        let offset = reader.offset();
        let mut current = S::ZERO;
        let mut exhausted = false;

        while dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut current, &size, &mut exhausted) {
            let _ = I::parse(reader, options)?;
        }

        Ok(Self {
            size,
            offset,
            _pd: core::marker::PhantomData,
        })
    }
}

pub enum DynamicArrayParse<'a, R: PollReader + Unpin, S: AsyncParse + ArraySize, I: AsyncParse, const ZERO_RELATIVE: bool> {
    Waiting(usize, <S as AsyncParse>::Fut<'a, R>),
    Iterating {
        offset: usize,
        exhausted: bool,
        len: S,
        idx: S,
        fut: <I as AsyncParse>::Fut<'a, R>,
    },
    Done(R, &'a ParseOptions),
    Empty,
}

impl <'a, R: PollReader + Unpin, S: AsyncParse + ArraySize + Unpin, I: AsyncParse + Unpin, const ZERO_RELATIVE: bool> Future for DynamicArrayParse<'a, R, S, I, ZERO_RELATIVE> {
    type Output = Result<DynamicArray<S, I, ZERO_RELATIVE>, ParseError>;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        use core::task::Poll;
        use core::pin::Pin;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Waiting(offset, mut fut) => {
                    match Pin::new(&mut fut).poll(cx) {
                        Poll::Pending => {
                            *self = Self::Waiting(offset, fut);
                            return Poll::Pending;
                        }
                        Poll::Ready(s) => {
                            let (reader, options) = fut.take_reader();
                            *self = Self::Iterating {
                                offset,
                                len: s?,
                                idx: S::ZERO,
                                exhausted: false,
                                fut: I::create_fut(reader, options)
                            }
                        }
                    }
                }
                Self::Iterating {
                    offset,
                    mut exhausted,
                    len,
                    mut idx,
                    mut fut,
                } => {
                    match Pin::new(&mut fut).poll(cx) {
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
                            let _ = i?;
                            let has_next = dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut idx, &len, &mut exhausted);
                            let (reader, opts) = fut.take_reader();
                            if has_next {
                                *self = Self::Iterating {
                                    offset,
                                    exhausted,
                                    len,
                                    idx,
                                    fut: I::create_fut(reader, opts)
                                };
                                continue;
                            }
                            *self = Self::Done(reader, opts);
                            return Poll::Ready(Ok(DynamicArray {
                                offset,
                                size: len,
                                _pd: core::marker::PhantomData,
                            }))
                        }
                    }
                }
                Self::Done(..) => panic!("polled after completion"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl <'a, R: PollReader + Unpin, S: AsyncParse + ArraySize, I: AsyncParse, const ZERO_RELATIVE: bool> TakeReader<'a, R> for DynamicArrayParse<'a, R, S, I, ZERO_RELATIVE> {
    impl_take_reader!{}
    fn borrow_reader(&mut self) -> &mut R {
        match self {
            Self::Waiting(_, f) => f.borrow_reader(),
            Self::Iterating {
                fut,
                ..
            } => fut.borrow_reader(),
            Self::Done(r, _) => r,
            Self::Empty => unreachable!(),
        }
    }
}

impl <S: AsyncParse + ArraySize + Unpin, I: AsyncParse + Unpin, const ZERO_RELATIVE: bool> AsyncParse for DynamicArray<S, I, ZERO_RELATIVE> {
    type Fut<'a, R: PollReader + Unpin> = DynamicArrayParse<'a, R, S, I, ZERO_RELATIVE>;
    fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
        let offset = reader.offset();
        DynamicArrayParse::Waiting(offset, S::create_fut(reader, options))
    }
}

pub trait FlagsParse<B>: Sized {
    fn try_from_bits(bits: B, options: &ParseOptions) -> Result<Self, ParseError>;
}

pub trait Flags<B> {
    fn from_bits(bits: B) -> Self;
    fn to_bits(self) -> B;
}

#[cfg(test)]
mod test {
    use super::*;
    use atom_parser_derive::make_atom;
    make_atom! {
        // TODO
        //  - better module resolution for parser
        //      - e.g, either use ::mod_name:: or self::mod_name so types cannot get confused.
        //      - that also means that the test impl would have to be moved to another crate
        //          - this is probably better for testing anyway
    }
}

/*
mod atoms {
    use super::*;
    use atom_parser_derive::make_atom;

    make_atom! {
        #[atom("root")]
        struct Root {
            #[children]
            enum Child {
                FileType,
                Movie,
            }
        }

        #[atom("ftyp")]
        struct FileType {
            major_brand: FourCC,
            minor_version: u32,
            #[trailing_array]
            compatible_brands: FourCC,
        }

        #[atom("moov")]
        struct Movie {
            #[children]
            enum Child {
                MovieHeader,
                Clipping,
                Track,
                Userdata,
                ColorTable,
                CompressedMovie,
                ReferenceMovie,
            }
        }
    }

    // empty atoms
    make_atom! {
        #[atom("wide")]
        struct Wide {}

        #[atom("free")]
        struct Free {
            #[payload]
            free_space: Vec<u8>,
        }

        #[atom("skip")]
        struct Skip {
            #[payload]
            free_space: Vec<u8>,
        }
    }

    // this can kinda just be anywhere and contain anything
    make_atom! {
        #[atom("udta")]
        struct Userdata {

        }
    }

    // compressed movie data
    make_atom! {
        #[atom("cmov")]
        struct CompressedMovie {
            #[children]
            enum Child {
                DataCompression,
                CompressedMovieData,
            }
        }

        #[atom("dcom")]
        struct DataCompression {
            compression_algorithm: u32,
        }

        #[atom("cmvd")]
        struct CompressedMovieData {
            #[payload]
            compressed_movie_data: Vec<u8>,
        }
    }


    // reference movies
    make_atom! {
        #[atom("rmra")]
        struct ReferenceMovie {
            #[trailing_array]
            reference_movie_descriptors: ReferenceMovieDescriptor,
        }

        #[atom("rmda")]
        struct ReferenceMovieDescriptor {
            #[children]
            enum Child {
                DataReference2,
                CPUSpeed,
                VersionCheck,
                ComponentDetect,
                Quality,
            }
        }

        #[atom("rdrf")]
        struct DataReference2 {
            #[flags(u32)]
            flags: struct Flags {
                const SELF_CONTAINED = 1 << 0;
            },
            data_reference: dref::v0::Child,
        }

        #[atom("rmdr")]
        struct DataRate {
            #[flags(u32)]
            flags: struct Flags {
                // empty
            },
            data_rate: u32,
        }

        #[atom("rmcs")]
        struct CPUSpeed {
            #[flags(u32)]
            flags: struct Flags {
                // empty
            },
            cpu_speed: u32,
        }

        #[atom("rmvc")]
        struct VersionCheck {
            #[flags(u32)]
            flags: struct Flags {
                // empty
            },
            software_package: u32,
            version: u32,
            mask: u32,
            check_type: u16,
        }

        #[atom("rmcd")]
        struct ComponentDetect {
            #[flags(u32)]
            flags: struct Flags {
                // empty
            },
            component_description: ComponentDescription,
            minimum_version: u32,
        }

        struct ComponentDescription {
            component_type: FourCC,
            component_subtype: FourCC,
            component_manufacturer: FourCC,
            component_flags: u32,
            component_flags_mask: u32,
        }

        #[atom("rmqu")]
        struct Quality {
            quality: u32,
        }
    }

    // stuff in movie
    make_atom! {
        #[version(0)]
        #[atom("mvhd")]
        struct MovieHeader {
            #[full_box]
            struct Flags {
                // empty
            },
            creation_time: u32,
            modification_time: u32,
            time_scale: u32,
            duration: u32,
            preferred_rate: u32,
            preferred_volume: u16,
            #[reserved]
            reserved: [u8; 10],
            display_matrix: [[u32; 3]; 3],
            preview_time: u32,
            preview_duration: u32,
            poster_time: u32,
            selection_time: u32,
            selection_duration: u32,
            current_time: u32,
            next_track_id: u32,
        }

        #[atom("ctab")]
        struct ColorTable {
            seed: u32,
            #[flags(u16)]
            flags: struct Flags {
                #[expected]
                const EXPECTED = 0x8000;
            },
            #[dynamic_array(size_type = u16, zero_relative = true)]
            color_table: Color,
        }

        struct Color {
            #[reserved]
            reserved: u16,
            red: u16,
            green: u16,
            blue: u16,
        }

        #[atom("trak")]
        struct Track {
            #[children]
            enum Child {
                TrackHeader,
                Clipping,
                TrackMatte,
                Edit,
                TrackReference,
                TrackLoadingSettings,
                TrackInputMap,
                Media,
                Userdata,
            }
        }
    }

    // clipping region
    // can be used in movies/tracks
    make_atom! {
        #[atom("clip")]
        struct Clipping {
            #[children]
            enum Child {
                ClippingRegion,
            }
        }

        #[atom("crgn")]
        struct ClippingRegion {
            region_size: u16,
            boundary_box: [u8; 8],
            #[payload]
            clipping_region: Vec<u8>
        }
    }

    // track specific
    make_atom! {
        #[version(0)]
        #[atom("tkhd")]
        struct TrackHeader {
            #[full_box]
            struct Flags {
                const ENABLED = 1 << 0;
                const USED = 1 << 1;
                const USED_IN_PREVIEW = 1 << 2;
                const USED_IN_POSTER = 1 << 3;
            },
            creation_time: u32,
            modification_time: u32,
            track_id: u32,
            #[reserved]
            reserved: [u8; 4],
            duration: u32,
            #[reserved]
            reserved: [u8; 8],
            layer: u16,
            alternate_group: u16,
            volume: u16,
            #[reserved]
            reserved: [u8; 2],
            matrix: [[u32; 3]; 3],
            track_width: u32,
            track_height: u32,
        }


        #[atom("matt")]
        struct TrackMatte {
            #[children]
            enum Child {
                CompressedMatte,
            }
        }
        #[version(0)]
        #[atom("kmat")]
        struct CompressedMatte {
            #[full_box]
            struct Flags {
                // unused
            },
            // TODO:
            // technically any video description can be here
        }

        #[atom("edts")]
        struct Edit {
            #[children]
            enum Child {
                EditList,
            }
        }

        #[version(0)]
        #[atom("elst")]
        struct EditList {
            #[full_box]
            struct Flags {
                // empty
            },
            #[dynamic_array(size_type = u32, zero_relative = false)]
            table_entries: EditListEntry,
        }

        struct EditListEntry {
            duration: u32,
            media_time: u32,
            media_rate: u32,
        }

        #[atom("tref")]
        struct TrackReference {
            #[children]
            enum Child {
                TimeCode,
                ChapterList,
                Synchonization,
                Transcript,
                NonprimarySource,
                Hint,
            }
        }

        #[atom("tmcd")]
        struct TimeCode {
            #[trailing_array]
            related_track_ids: u32,
        }

        #[atom("chap")]
        struct ChapterList {
            #[trailing_array]
            related_track_ids: u32,
        }

        #[atom("sync")]
        struct Synchonization {
            #[trailing_array]
            related_track_ids: u32,
        }

        #[atom("scpt")]
        struct Transcript {
            #[trailing_array]
            related_track_ids: u32,
        }

        #[atom("ssrc")]
        struct NonprimarySource {
            #[trailing_array]
            related_track_ids: u32,
        }

        #[atom("hint")]
        struct Hint {
            #[trailing_array]
            related_track_ids: u32,
        }


        #[atom("load")]
        struct TrackLoadingSettings {
            preload_start_time: u32,
            preload_duration: u32,
            #[flags(u32)]
            preload_flags: struct Flags {
                const PRELOADED_REGARDLESS = 1 << 0;
                const PRELOAD_IF_ENABLED = 1 << 1;
            },
            default_hints: u32,
        }

        #[atom("imap")]
        struct TrackInputMap {
            #[children]
            enum Child {
                TrackInput,
            }
        }

        #[atom(b"\0\0in")]
        struct TrackInput {
            id: u32,
            #[reserved]
            reserved: [u8; 2],
            child_count: u16,
            #[reserved]
            reserved: [u8; 4],
            #[children]
            enum Child {
                InputType,
                ObjectId,
            }
        }

        #[atom(b"\0\0ty")]
        struct InputType {
            ty: u32,
        }

        #[atom("obid")]
        struct ObjectId {
            object_id: u32,
        }

        #[atom("mdia")]
        struct Media {
            #[children]
            enum Child {
                MediaHeader,
                HandlerReference,
                MediaInformation,
                Userdata,
            }
        }
    }

    //media
    make_atom! {
        #[version(0)]
        #[atom("mdhd")]
        struct MediaHeader {
            #[full_box]
            struct Flags {
                // empty
            },
            creation_time: u32,
            modification_time: u32,
            time_scale: u32,
            duration: u32,
            language: u16,
            quality: u16,
        }

        #[version(0)]
        #[atom("hdlr")]
        struct HandlerReference {
            #[full_box]
            struct Flags {
                // empty
            },
            component_type: u32,
            component_subtype: u32,
            #[reserved]
            component_manufacturer: u32,
            #[reserved]
            component_flags: u32,
            #[reserved]
            component_flags_mask: u32,
            #[pascal_string(u8)]
            component_name: String,
        }
        #[atom("minf")]
        struct MediaInformation {
            #[children]
            enum Child {
                VideoMediaInformationHeader,
                SoundMediaInformationHeader,
                TimecodeMediaInformation,
                BaseMediaInformationHeader,
                BaseMediaInformation,
                HandlerReference,
                DataInformation,
                SampleTable,
            }
        }
    }

    // media information
    make_atom! {
        #[version(0)]
        #[atom("vmhd")]
        struct VideoMediaInformationHeader {
            #[full_box]
            struct Flags {
                const NO_LEAN_AHEAD = 1 << 0;
            },
            graphics_mode: u16,
            opcolor: [u16; 3],
        }

        #[version(0)]
        #[atom("smhd")]
        struct SoundMediaInformationHeader {
            #[full_box]
            struct Flags {
                // empty
            },
            balance: u16,
            #[reserved]
            resered: [u8; 2]
        }

        #[atom("gmhd")]
        struct BaseMediaInformationHeader {
            // empty...
        }

        #[version(0)]
        #[atom("gmin")]
        struct BaseMediaInformation {
            #[full_box]
            struct Flags {
                // empty
            },
            graphics_mode: u16,
            opcolor: [u16; 3],
            balance: u16,
            #[reserved]
            reserved: [u8; 2]
        }

        #[version(0)]
        #[atom("tmci")]
        struct TimecodeMediaInformation {
            #[full_box]
            struct Flags {
                // empty
            },
            text_font: u16,
            #[flags(u16)]
            text_face: struct TextFace {
                const BOLD = 1 << 0;
                const ITALIC = 1 << 1;
                const UNDERLINE = 1 << 2;
                const OUTLINE = 1 << 3;
                const SHADOW = 1 << 4;
                const CONDENSE = 1 << 5;
                const EXTEND = 1 << 6;
            },
            text_size: u16,
            text_color: [u16; 3],
            background_color: [u16; 3],
            #[pascal_string(u8)]
            font_name: String,
        }
    }

    // sample table
    make_atom! {
        #[atom("stbl")]
        struct SampleTable {
            #[children]
            enum Child {
                SampleDescription,
                TimeToSample,
                SyncSample,
                SampleToChunk,
                SampleSize,
                ChunkOffset,
                // ShadowSync, reserved
            },
        }

        struct SampleDescriptionEntry<D> {
            #[reserved]
            reserved: [u8; 6],
            data_reference_index: u16,
            description: D,
        }

        #[version(0)]
        #[atom("stsd")]
        struct SampleDescription {
            #[full_box]
            struct Flags {
                // empty
            },
            #[children(u32)]
            enum Child {
                // TODO
            }
        }

        #[version(0)]
        #[atom("stts")]
        struct TimeToSample {
            #[full_box]
            struct Flags {
                // empty
            },
            #[dynamic_array(size_type = u32, zero_relative = false)]
            time_to_sample_table: TimeToSampleTableEntry,
        }

        struct TimeToSampleTableEntry {
            sample_count: u32,
            sample_duration: u32,
        }

        #[version(0)]
        #[atom("stss")]
        struct SyncSample {
            #[full_box]
            struct Flags {
                // empty
            },
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sync_sample_table: u32,
        }

        #[version(0)]
        #[atom("stsc")]
        struct SampleToChunk {
            #[full_box]
            struct Flags {
                // empty
            },
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sample_to_chunk_table: SampleToChunkTableEntry,
        }

        struct SampleToChunkTableEntry {
            first_chunk: u32,
            samples_per_chunk: u32,
            sample_description_id: u32,
        }

        #[version(0)]
        #[atom("stsz")]
        struct SampleSize {
            #[full_box]
            struct Flags {
                // empty
            },
            sample_size: u32,
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sample_size_table: u32,
        }
    }

    // data information
    make_atom! {
        #[atom("dinf")]
        struct DataInformation {
            #[children]
            enum Child {
                DataReference,
            }
        }

        #[version(0)]
        #[atom("dref")]
        struct DataReference {
            #[full_box]
            struct Flags {
                // empty
            },
            #[children(u32)]
            enum Child {
                MacAlias,
                MacResource,
                Url,
            }
        }

        #[version(0)]
        #[atom("alis")]
        struct MacAlias {
            #[full_box]
            struct Flags {
                const SELF_REFERENTIAL = 1 << 0;
            },
            #[payload]
            mac_alias: Vec<u8>,
        }

        #[version(0)]
        #[atom("rsrc")]
        struct MacResource {
            #[full_box]
            struct Flags {
                const SELF_REFERENTIAL = 1 << 0;
            },
            #[payload]
            mac_alias_resource: Vec<u8>,
        }

        #[version(0)]
        #[atom(b"url\0")]
        struct Url {
            #[full_box]
            struct Flags {
                const SELF_REFERENTIAL = 1 << 0;
            },
            #[payload]
            url: Vec<u8>,
        }

        #[version(0)]
        #[atom("stco")]
        struct ChunkOffset {
            #[full_box]
            struct Flags {
                // empty
            },
            #[dynamic_array(size_type = u32, zero_relative = false)]
            chunk_offset_table: u32,
        }
    }

    // video media
    pub mod sample_description {
        use super::*;
        make_atom! {
            struct Video {
                version: u16,
                #[reserved]
                revision_level: u16,
                vendor: u32,
                temporal_quality: u32,
                spatial_quality: u32,
                width: u16,
                height: u16,
                horizontal_resolution: u32,
                vertical_resolution: u32,
                #[reserved]
                data_size: u32,
                // frames of data per sample
                frame_count: u16,
                #[pascal_string(u32)]
                compressor_name: String,
                pixel_depth: u32,
                color_table_id: u16,
            }
            // TODO: mjpeg stuff?
            // - see page 99


            struct Sound {
                version: u16,
                revision_level: u16,
                vendor: u32,
                number_of_channels: u16,
                sample_size: u16,
                compression_id: u16,
                packet_size: u16,
                sample_rate: u32,
            }

            struct Timecode {
                #[reserved]
                reserved: u32,
                #[flags(u32)]
                flags: struct Flags {
                    const DROP_FRAME = 1 << 0;
                    const WRAP_AFTER_24H = 1 << 1;
                    const SUPPORTS_NEGATIVE_TIME = 1 << 2;
                    const IS_TAPE_COUNTER = 1 << 3;
                },
                time_scale: u32,
                frame_duration: u32,
                number_of_frames: u8,
                #[reserved]
                reserved: [u8; 3],
                source_reference: Userdata,
            }

            struct Text {
                #[flags(u32)]
                display_flags: struct DisplayFlags {
                    const DONT_AUTO_SCALE = 1 << 1;
                    const USE_MOVIE_BACKGROUND_COLOR = 1 << 4;
                    const SCROLL_IN = 1 << 5;
                    const SCROLL_OUT = 1 << 6;
                    const HORIZONTAL_SCROLL = 1 << 7;
                    const REVERSE_SCROLL = 1 << 8;
                    const CONTINUOUS_SCROLL = 1 << 9;
                    const DROP_SHADOW = 1 << 12;
                    const ANTI_ALIAS = 1 << 13;
                    const KEY_TEXT = 1 << 14;
                },
                text_justification: u32,
                default_text_box: u64,
                #[reserved]
                reserved: [u8; 8],
                font_number: u16,
                #[flags(u16)]
                font_face: struct FontFace {
                    const BOLD = 1 << 0;
                    const ITALIC = 1 << 1;
                    const UNDERLINE = 1 << 2;
                    const OUTLINE = 1 << 3;
                    const SHADOW = 1 << 4;
                    const CONDENSE = 1 << 5;
                    const EXTEND = 1 << 6;
                },
                #[reserved]
                reserved: u8,
                #[reserved]
                reserved: [u8; 2],
                foreground_color: [u16; 3],
                #[pascal_string(u8)]
                text_name: String,
            }
            // TODO: text sample extensions / hypertext
            // - see page 111

            struct Music {
                #[flags(u32)]
                flags: struct Flags {
                    // empty
                }
            }

            struct Mpeg {
                // empty
            }

            struct Sprite {
                // empty
            }
            
            struct Tween {
                // empty
            }

            struct Q3D {
                // empty
            }

            struct Streaming {
                version: u32,
                #[reserved]
                reserved: [u8; 4],
                flags: u32,
            }

            struct Hint {
                version: u16,
                last_compatible_version: u16,
                max_packet_size: u32,
                // TODO: children for rtp
                // - see page 151
            }
        }
    }

    type VideoSampleDescription = SampleDescriptionEntry<sample_description::Video>;
    type SoundSampleDescription = SampleDescriptionEntry<sample_description::Sound>;
    type TimecodeSampleDescription = SampleDescriptionEntry<sample_description::Timecode>;
    type TextSampleDescription = SampleDescriptionEntry<sample_description::Text>;
    type MusicSampleDescription = SampleDescriptionEntry<sample_description::Music>;
    type MpegSampleDescription = SampleDescriptionEntry<sample_description::Mpeg>;
    type SpriteSampleDescription = SampleDescriptionEntry<sample_description::Sprite>;
    type TweenSampleDescription = SampleDescriptionEntry<sample_description::Sprite>;
    type Q3DSampleDescription = SampleDescriptionEntry<sample_description::Q3D>;
    type StreamingSampleDescription = SampleDescriptionEntry<sample_description::Streaming>;


    // hints
    make_atom! {
        #[atom("hnti")]
        struct HintInfo {
            #[children]
            enum Child {

            }
        }

        #[atom("trpy")]
        struct TrackPayloadSizeHint64 {
            byte_len: u64,
        }
        #[atom("totl")]
        struct TrackPayloadSizeHint32 {
            byte_len: u32,
        }
        #[atom("nump")]
        struct NetworkPacketHint64 {
            network_packet_count: u64,
        }
        #[atom("npck")]
        struct NetworkPacketHint32 {
            network_packet_count: u32,
        }
        #[atom("tpyl")]
        struct ByteCountHint64 {
            total_bytes_minus_rtp_headers: u64,
        }
        #[atom("tpay")]
        struct ByteCountHint32 {
            total_bytes_minus_rtp_headers: u32,
        }
        #[atom("maxr")]
        struct DatarateHint {
            granularity_ms: u32,
            maximum_datarate: u32,
        }
        #[atom("dmed")]
        struct MediaTrackBytesHint {
            byte_count: u64,
        }
        #[atom("dimm")]
        struct ImmediateBytesHint {
            byte_count: u64,
        }
        #[atom("drep")]
        struct RepeatedBytesHint {
            byte_count: u64,
        }
        #[atom("tmin")]
        struct MinTransmissionTimeHint {
            shortest_transmission_ms: u32,
        }
        #[atom("tmax")]
        struct MaxTransmissionTimeHint {
            longest_transmission_ms: u32,
        }
        #[atom("pmax")]
        struct LargestPacketHint {
            byte_count: u32,
        }
        #[atom("dmax")]
        struct LargestPacketDurationHint {
            largest_duration_ms: u32,
        }
        #[atom("payt")]
        struct PayloadTypeHint {
            payload_number: u32,
            #[pascal_string(u8)]
            rtpmap: String,
        }
    }

    #[test]
    fn ftyp() {
        let mut r = InMemoryReader::from_path("../file_example_MOV_480_700kB.mov").unwrap();
        let opts = Default::default();

        use Parse;

        let root = Root::parse(&mut r, &opts).unwrap();
        let mut root_iter = root.children(&mut r, &opts);

        while let Some(child) = root_iter.next() {
            let r = &mut root_iter.reader;
            match child.unwrap() {
                root::Child::FileType(ftyp) => println!("file type: {ftyp:?}"),
                root::Child::Movie(movie) => {
                    println!("moov: {movie:?}");
                    let mut child_iter = movie.children(r, &opts);

                    while let Some(child) = child_iter.next() {
                        let r = &mut child_iter.reader;
                        match child.unwrap() {
                            moov::Child::MovieHeader(hd) => {
                                println!("header: {hd:?}");
                            }
                            moov::Child::Clipping(clip) => {
                                println!("clip: {clip:?}");
                            }
                            moov::Child::Track(track) => {
                                println!("track: {track:?}");
                                let mut child_iter = track.children(r, &opts);
                                while let Some(child) = child_iter.next() {
                                    let r = &mut child_iter.reader;
                                    match child.unwrap() {
                                        trak::Child::TrackHeader(hdr) => println!("track header: {hdr:?}"),
                                        trak::Child::Clipping(c) => println!("clipping: {c:?}"),
                                        trak::Child::TrackMatte(tm) => println!("track matte: {tm:?}"),
                                        trak::Child::Edit(e) => {
                                            println!("edit: {e:?}");
                                            let mut child_iter = e.children(r, &opts);
                                            while let Some(child) = child_iter.next() {
                                                let r = &mut child_iter.reader;
                                                match child.unwrap() {
                                                    edts::Child::EditList(el) => {
                                                        match el.version {
                                                            elst::EditListVersions::V0(el) => {
                                                                println!("edit list: {el:?}");
                                                                let list_entires = el.table_entries(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                println!("edit list entries: {list_entires:?}");
                                                            }
                                                            elst::EditListVersions::Unknown(v) => println!("unknown elst: {v}"),
                                                        }
                                                    }
                                                    _ => (),
                                                }
                                            }
                                        }
                                        trak::Child::TrackReference(tref) => println!("track ref: {tref:?}"),
                                        trak::Child::TrackLoadingSettings(tls) => println!("track loading: {tls:?}"),
                                        trak::Child::TrackInputMap(tim) => println!("track input map: {tim:?}"),
                                        trak::Child::Media(m) => {
                                            let mut child_iter = m.children(r, &opts);
                                            while let Some(child) = child_iter.next() {
                                                let r = &mut child_iter.reader;
                                                match child.unwrap() {
                                                    mdia::Child::MediaHeader(h) => println!("media heaader: {h:?}"),
                                                    mdia::Child::HandlerReference(r) => println!("href: {r:?}"),
                                                    mdia::Child::MediaInformation(info) => {
                                                        println!("info: {info:?}");
                                                        let mut child_iter = info.children(r, &opts);
                                                        while let Some(child) = child_iter.next() {
                                                            let r = &mut child_iter.reader;
                                                            match child.unwrap() {
                                                                minf::Child::VideoMediaInformationHeader(vid) => println!("vid: {vid:?}"),
                                                                minf::Child::SoundMediaInformationHeader(snd) => println!("snd: {snd:?}"),
                                                                minf::Child::TimecodeMediaInformation(info) => println!("timecode: {info:?}"),
                                                                minf::Child::BaseMediaInformationHeader(base) => println!("base: {base:?}"),
                                                                minf::Child::BaseMediaInformation(base) => println!("base info: {base:?}"),
                                                                minf::Child::HandlerReference(href) => println!("href: {href:?}"),
                                                                minf::Child::DataInformation(dinfo) => {
                                                                    println!("dinfo: {dinfo:?}");
                                                                    let mut child_iter = dinfo.children(r, &opts);
                                                                    while let Some(child) = child_iter.next() {
                                                                        let r = &mut child_iter.reader;
                                                                        match child.unwrap() {
                                                                            dinf::Child::DataReference(dref) => {
                                                                                match dref.version {
                                                                                    dref::DataReferenceVersions::V0(dref) => {
                                                                                        let mut child_iter = dref.children(r, &opts);
                                                                                        while let Some(child) = child_iter.next() {
                                                                                            match child.unwrap() {
                                                                                                dref::v0::Child::MacAlias(alis) => println!("alias: {alis:?}"),
                                                                                                dref::v0::Child::MacResource(rsrc) => println!("r: {rsrc:?}"),
                                                                                                dref::v0::Child::Url(url) => println!("url: {url:?}"),
                                                                                                _ => (),
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                    dref::DataReferenceVersions::Unknown(v) => println!("unknown dref: {v}"),
                                                                                }
                                                                            }
                                                                            _ => (),
                                                                        }
                                                                    }
                                                                }
                                                                minf::Child::SampleTable(stable) => {
                                                                    println!("stable: {stable:?}");
                                                                    let mut child_iter = stable.children(r, &opts);
                                                                    while let Some(child) = child_iter.next() {
                                                                        let r = &mut child_iter.reader;
                                                                        match child.unwrap() {
                                                                            stbl::Child::SampleDescription(desc) => {
                                                                                match desc.version {
                                                                                    stsd::SampleDescriptionVersions::V0(desc) => {
                                                                                        println!("{desc:?}");
                                                                                        let mut child_iter = desc.children(r, &opts);
                                                                                        while let Some(child) = child_iter.next() {
                                                                                            let r = &mut child_iter.reader;
                                                                                            println!("sample desc: {child:?}");
                                                                                        }
                                                                                    }
                                                                                    stsd::SampleDescriptionVersions::Unknown(v) => println!("unknown stsd: {v}"),
                                                                                }
                                                                            }
                                                                            stbl::Child::TimeToSample(tts) => {
                                                                                match tts.version {
                                                                                    stts::TimeToSampleVersions::V0(tts) => {
                                                                                        println!("tts: {tts:?}");
                                                                                        let time_to_sample = tts.time_to_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                        println!("tts entries: {time_to_sample:?}");
                                                                                    }
                                                                                    stts::TimeToSampleVersions::Unknown(v) => println!("unknown stts: {v}"),
                                                                                }
                                                                            }
                                                                            stbl::Child::SyncSample(sync) => {
                                                                                match sync.version {
                                                                                    stss::SyncSampleVersions::V0(sync) => {
                                                                                        println!("sync sample: {sync:?}");
                                                                                        let sync_samples = sync.sync_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                        println!("sync sample entries: {sync_samples:?}");
                                                                                    }
                                                                                    stss::SyncSampleVersions::Unknown(v) => println!("unknown stts: {v}"),
                                                                                }
                                                                            }
                                                                            stbl::Child::SampleToChunk(stc) => {
                                                                                match stc.version {
                                                                                    stsc::SampleToChunkVersions::V0(stc) => {
                                                                                        println!("stc: {stc:?}");
                                                                                        let sample_to_chunk = stc.sample_to_chunk_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                        println!("stc entries: {sample_to_chunk:?}");
                                                                                    }
                                                                                    stsc::SampleToChunkVersions::Unknown(v) => println!("unknown stsc: {v}"),
                                                                                }
                                                                            }
                                                                            stbl::Child::SampleSize(ss) => {
                                                                                match ss.version {
                                                                                    stsz::SampleSizeVersions::V0(ss) => {
                                                                                        println!("ss: {ss:?}");
                                                                                        let sample_sizes = ss.sample_size_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                        println!("ss entries: {sample_sizes:?}");
                                                                                    }
                                                                                    stsz::SampleSizeVersions::Unknown(v) => println!("unknown stsz: {v}"),
                                                                                }
                                                                            }
                                                                            stbl::Child::ChunkOffset(co) => {
                                                                                match co.version {
                                                                                    stco::ChunkOffsetVersions::V0(co) => {
                                                                                        println!("co32: {co:?}");
                                                                                        let offsets = co.chunk_offset_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                        println!("co32 entries: {offsets:?}")
                                                                                    }
                                                                                    stco::ChunkOffsetVersions::Unknown(v) => println!("unknown stco: {v}"),
                                                                                }
                                                                            }
                                                                            stbl::Child::Unsupported(u) => println!("unsupported stbl entry: {u:?}"),
                                                                        }
                                                                    }
                                                                }
                                                                minf::Child::Unsupported(_) => (),
                                                            }
                                                        }
                                                    }
                                                    mdia::Child::Userdata(u) => println!("udata: {u:?}"),
                                                    mdia::Child::Unsupported(_) => (),
                                                }
                                            }
                                        }
                                        trak::Child::Userdata(udata) => println!("udata: {udata:?}"),
                                        trak::Child::Unsupported(fcc) => println!("unsupported in trak: {fcc:?}"),
                                    }
                                }
                            }
                            moov::Child::Userdata(udta) => {
                                println!("udata: {udta:?}");
                            }
                            moov::Child::ColorTable(ctb) => {
                                println!("color table: {ctb:?}");
                            }
                            moov::Child::Unsupported(missed) => {
                                println!("skipped: {missed:?}");
                            }
                            _ => (),
                        }
                    }
                }
                root::Child::Unsupported(a) => println!("unsupported {a:?}"),
            }
        }
        panic!()
    }
}
*/
