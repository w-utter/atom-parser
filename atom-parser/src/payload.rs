use crate::{Extent, ParseError, ParseOptions, PollReader, Reader, TakeReader, Offset};

#[derive(Debug)]
pub struct Payload<O, S = O> {
    offset: O,
    size: S,
}

impl<O: Offset, S: Copy> Payload<O, S> {
    pub fn extent(&self) -> Extent<O, S> {
        let Self { offset, size, .. } = self;

        Extent {
            offset: *offset,
            len: *size,
        }
    }
}

impl <O: Offset> Payload<O> {
    pub fn parse<T: Reader<O>>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self { offset, size })
    }
}

impl<O: Offset, S> Payload<O, S> {
    pub fn parse_from_len<T: Reader<O>>(
        size: S,
        reader: &mut T,
        _: &ParseOptions,
    ) -> Result<Self, ParseError> {
        let offset = reader.offset();
        Ok(Self { offset, size })
    }
}

pub struct PayloadParse<'a, O: Unpin, R, S = O>(R, &'a ParseOptions, S, core::marker::PhantomData<O>);
impl<'a, O: Offset + Unpin, R: PollReader<O> + Unpin, S: Copy + Unpin> Future for PayloadParse<'a, O, R, S> {
    type Output = Result<Payload<O, S>, ParseError>;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        _: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let offset = self.0.offset();
        let size = self.2;
        core::task::Poll::Ready(Ok(Payload { offset, size }))
    }
}

impl<'a, O: Unpin, R, S> TakeReader<'a, R> for PayloadParse<'a, O, R, S> {
    fn take_reader(self) -> (R, &'a ParseOptions) {
        (self.0, self.1)
    }
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        (&mut self.0, self.1)
    }
}

impl <O: Offset + Unpin> Payload<O> {
    pub fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> PayloadParse<'a, O, R> {
        let len = reader.remaining_size();
        PayloadParse(reader, options, len, core::marker::PhantomData)
    }
}

impl<O: Offset + Unpin, S> Payload<O, S> {
    pub fn create_fut_from_len<'a, R: PollReader<O> + Unpin>(
        len: S,
        reader: R,
        options: &'a ParseOptions,
    ) -> PayloadParse<'a, O, R, S> {
        PayloadParse(reader, options, len, core::marker::PhantomData)
    }
}
