use crate::{
    AsyncParse, Offset, Options, Parse, ParseError, Parsed, PollReader, Reader, TakeReader,
    impl_take_reader,
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct CharacterCode<const N: usize>(pub [u8; N]);

impl<const N: usize> CharacterCode<N> {
    pub const fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub const fn has_prefix<const M: usize>(&self, pre: &[u8; M]) -> bool {
        assert!(M <= 4);

        let mut i = 0;
        while i < M {
            if self.0[i] != pre[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    pub fn has_postfix<const M: usize>(&self, post: &[u8; M]) -> bool {
        assert!(M <= 4);
        let mut i = 4 - M;
        let mut j = 0;
        while i < 4 {
            if self.0[i] != post[j] {
                return false;
            }
            i += 1;
            j += 1;
        }
        true
    }

    pub const fn subslice<const START: usize, const LEN: usize>(self) -> CharacterCode<LEN> {
        assert!(LEN <= 4);
        assert!(START + LEN <= 4);
        let mut arr = [0u8; LEN];
        let mut i = START;
        let mut j = 0;

        while i < 4 {
            arr[j] = self.0[i];
            i += 1;
            j += 1;
        }
        CharacterCode::new(arr)
    }

    pub const fn prefix<const LEN: usize>(self) -> CharacterCode<LEN> {
        self.subslice::<0, LEN>()
    }
}

impl<const N: usize> core::fmt::Debug for CharacterCode<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "cc\"")?;
        for chunk in self.0.utf8_chunks() {
            for ch in chunk.valid().chars() {
                write!(f, "{}", ch.escape_debug())?;
            }

            for byte in chunk.invalid() {
                write!(f, "\\x{:02X}", byte).unwrap();
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}

impl<const N: usize> Parsed for CharacterCode<N> {
    type Output<O> = CharacterCode<N>;
}

impl<const N: usize, O: Offset> Parse<O> for CharacterCode<N> {
    fn parse<T: Reader<O>, P: Options>(reader: &mut T, _: P) -> Result<Self, ParseError> {
        let mut buf = [0; N];

        reader.read(&mut buf)?;
        Ok(CharacterCode::new(buf))
    }
}

pub enum AsyncCharacterCodeParse<
    const N: usize,
    P: Options + Unpin,
    O: Offset + Unpin,
    R: PollReader<O> + Unpin,
> {
    Waiting(R, P, [u8; N], core::marker::PhantomData<O>),
    Done(R, P),
    Empty,
}

impl<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin, const N: usize>
    TakeReader<P, R> for AsyncCharacterCodeParse<N, P, O, R>
{
    impl_take_reader! {}

    fn borrow_reader(&mut self) -> (&mut R, &P) {
        match self {
            Self::Waiting(r, opts, _, _) => (r, opts),
            Self::Done(r, opts) => (r, opts),
            Self::Empty => unreachable!(),
        }
    }
}

impl<const N: usize, P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> Future
    for AsyncCharacterCodeParse<N, P, O, R>
{
    type Output = Result<CharacterCode<N>, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let this = core::mem::replace(&mut *self, Self::Empty);
        match this {
            Self::Waiting(mut r, opts, mut buf, _) => {
                match core::pin::Pin::new(&mut r).poll_read(cx, &mut buf) {
                    core::task::Poll::Pending => {
                        *self = Self::Waiting(r, opts, buf, core::marker::PhantomData);
                        return core::task::Poll::Pending;
                    }
                    core::task::Poll::Ready(res) => {
                        let _ = res?;
                        *self = Self::Done(r, opts);
                        return core::task::Poll::Ready(Ok(CharacterCode::new(buf)));
                    }
                }
            }
            Self::Done(..) => panic!("future polled after completion"),
            Self::Empty => unreachable!(),
        }
    }
}

impl<const N: usize, O: Offset + Unpin> AsyncParse<O> for CharacterCode<N> {
    type Fut<P: Options + Unpin, R: PollReader<O> + Unpin> = AsyncCharacterCodeParse<N, P, O, R>;
    fn create_fut<P: Options + Unpin, R: PollReader<O> + Unpin>(
        reader: R,
        options: P,
    ) -> Self::Fut<P, R> {
        AsyncCharacterCodeParse::Waiting(reader, options, [0u8; N], core::marker::PhantomData)
    }
}
