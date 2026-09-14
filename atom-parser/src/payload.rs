use crate::{AsyncParse, Extent, Parse, ParseError, ParseOptions, PollReader, Reader, TakeReader};

#[derive(Debug)]
pub struct Payload {
    offset: usize,
    size: usize,
}

impl Payload {
    pub fn extent(&self) -> Extent<usize, usize> {
        let Self { offset, size } = self;

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
        Ok(Self { offset, size })
    }
}

pub struct PayloadParse<'a, R>(R, &'a ParseOptions);
impl<'a, R: PollReader + Unpin> Future for PayloadParse<'a, R> {
    type Output = Result<Payload, ParseError>;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.0.remaining_size();
        core::task::Poll::Ready(Ok(Payload { offset, size }))
    }
}

impl<'a, R> TakeReader<'a, R> for PayloadParse<'a, R> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        (&mut self.0, self.1)
    }
}

impl AsyncParse for Payload {
    type Fut<'a, R: PollReader + Unpin> = PayloadParse<'a, R>;
    fn create_fut<'a, R: PollReader + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R> {
        PayloadParse(reader, options)
    }
}
