use crate::{
    AsyncParse, Endianess, IntegerParse, Offset, Options, Parse, ParseError, Parsed, PollReader,
    Reader, TakeReader,
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct FourCC([u8; 4]);

impl FourCC {
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    pub const fn from_le_bits(bits: u32) -> Self {
        Self::new(bits.to_le_bytes())
    }

    pub const fn from_be_bits(bits: u32) -> Self {
        Self::new(bits.to_be_bytes())
    }
}

impl core::fmt::Debug for FourCC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "fcc\"")?;
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

impl Parsed for FourCC {
    type Output<O> = FourCC;
}

impl<O: Offset> Parse<O> for FourCC {
    fn parse<T: Reader<O>, P: Options>(reader: &mut T, options: P) -> Result<Self, ParseError> {
        let inner = u32::parse(reader, &options)?;

        Ok(Self(match options.endianess() {
            Endianess::Little => inner.to_le_bytes(),
            Endianess::Big => inner.to_be_bytes(),
        }))
    }
}

pub struct FourCCParse<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> {
    pub inner: <u32 as AsyncParse<O>>::Fut<P, R>,
}

impl<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> Future
    for FourCCParse<P, O, R>
{
    type Output = Result<FourCC, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut this = core::mem::replace(
            &mut *self,
            FourCCParse {
                inner: IntegerParse::Empty,
            },
        );
        use core::task::Poll;
        match core::pin::Pin::new(&mut this.inner).poll(cx) {
            Poll::Pending => {
                core::mem::swap(&mut this, &mut *self);
                return Poll::Pending;
            }
            Poll::Ready(res) => {
                core::mem::swap(&mut this, &mut *self);
                let (_, opts) = self.borrow_reader();
                return Poll::Ready(res.map(|num| {
                    FourCC(match opts.endianess() {
                        Endianess::Big => num.to_be_bytes(),
                        Endianess::Little => num.to_le_bytes(),
                    })
                }));
            }
        }
    }
}

impl<P: Options + Unpin, O: Offset + Unpin, R: PollReader<O> + Unpin> TakeReader<P, R>
    for FourCCParse<P, O, R>
{
    fn take_reader(self) -> (R, P) {
        self.inner.take_reader()
    }
    fn borrow_reader(&mut self) -> (&mut R, &P) {
        self.inner.borrow_reader()
    }
}

impl<O: Offset + Unpin> AsyncParse<O> for FourCC {
    type Fut<P: Options + Unpin, R: PollReader<O> + Unpin> = FourCCParse<P, O, R>;
    fn create_fut<P: Options + Unpin, R: PollReader<O> + Unpin>(
        reader: R,
        options: P,
    ) -> Self::Fut<P, R> {
        FourCCParse {
            inner: u32::create_fut(reader, options),
        }
    }
}
