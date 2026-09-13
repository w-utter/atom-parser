use crate::{Parse, Reader, ParseOptions, PollReader, AsyncParse, TakeReader, ParseError, IntegerParse};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct FourCC([u8; 4]);

impl FourCC {
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
}

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
    pub inner: <u32 as AsyncParse>::Fut<'a, R>,
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
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
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
