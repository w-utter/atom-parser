use crate::{Extent, ParseError, ParseOptions, PollReader, Reader, TakeReader};

#[derive(Debug)]
pub struct Payload<S = usize> {
    offset: usize,
    size: S,
}

impl<S: Copy> Payload<S> {
    pub fn extent(&self) -> Extent<usize, S> {
        let Self { offset, size } = self;

        Extent {
            offset: *offset,
            len: *size,
        }
    }
}

impl Payload {
    pub fn parse<T: Reader>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self { offset, size })
    }
}

impl<S> Payload<S> {
    pub fn parse_from_len<T: Reader>(
        size: S,
        reader: &mut T,
        _: &ParseOptions,
    ) -> Result<Self, ParseError> {
        let offset = reader.offset();
        Ok(Self { offset, size })
    }
}

pub struct PayloadParse<'a, R, S = usize>(R, &'a ParseOptions, S);
impl<'a, R: PollReader + Unpin, S: Copy + Unpin> Future for PayloadParse<'a, R, S> {
    type Output = Result<Payload<S>, ParseError>;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.2;
        core::task::Poll::Ready(Ok(Payload { offset, size }))
    }
}

impl<'a, R, S> TakeReader<'a, R> for PayloadParse<'a, R, S> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        (&mut self.0, self.1)
    }
}

impl Payload {
    pub fn create_fut<'a, R: PollReader + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> PayloadParse<'a, R> {
        let len = reader.remaining_size();
        PayloadParse(reader, options, len)
    }
}

impl<S> Payload<S> {
    pub fn create_fut_from_len<'a, R: PollReader + Unpin>(
        len: S,
        reader: R,
        options: &'a ParseOptions,
    ) -> PayloadParse<'a, R, S> {
        PayloadParse(reader, options, len)
    }
}
