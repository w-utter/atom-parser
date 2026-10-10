use crate::{
    AsyncSeek, Extent, Offset, ParseError, ParseOptions, PollReader, Reader, TakeReader,
    TryFromIntError, impl_take_reader,
};

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

impl<O: Offset> Payload<O> {
    pub fn parse<T: Reader<O>>(reader: &mut T, _: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        reader.seek(size)?;
        Ok(Self { offset, size })
    }
}

impl<O: Offset + core::convert::TryFrom<S>, S: Copy> Payload<O, S> {
    pub fn parse_from_len<T: Reader<O>>(
        size: S,
        reader: &mut T,
        _: &ParseOptions,
    ) -> Result<Self, ParseError> {
        let offset = reader.offset();

        let payload_size =
            O::try_from(size).map_err(|_| ParseError::IntegerConversion(TryFromIntError))?;
        reader.seek(payload_size)?;

        Ok(Self { offset, size })
    }
}

pub enum PayloadParse<'a, O: Unpin, R, S = O> {
    Waiting(R, &'a ParseOptions, O, S),
    Seek(&'a ParseOptions, O, S, AsyncSeek<O, R>),
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, O: Offset + Unpin + core::convert::TryFrom<S>, R: PollReader<O> + Unpin, S: Copy + Unpin>
    Future for PayloadParse<'a, O, R, S>
{
    type Output = Result<Payload<O, S>, ParseError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Waiting(reader, opts, offset, size) => {
                    let payload_size = match O::try_from(size)
                        .map_err(|_| ParseError::IntegerConversion(TryFromIntError))
                    {
                        Ok(payload_size) => payload_size,
                        Err(e) => {
                            *self = Self::Done(reader, opts);
                            return core::task::Poll::Ready(Err(e));
                        }
                    };
                    let seek = AsyncSeek::seek(reader, payload_size);
                    *self = Self::Seek(opts, offset, size, seek);
                }
                Self::Seek(opts, offset, size, mut fut) => {
                    match core::pin::Pin::new(&mut fut).poll(cx) {
                        core::task::Poll::Pending => {
                            *self = Self::Seek(opts, offset, size, fut);
                            return core::task::Poll::Pending;
                        }
                        core::task::Poll::Ready(res) => {
                            *self = Self::Done(fut.reader, opts);
                            let _ = res?;
                            return core::task::Poll::Ready(Ok(Payload { offset, size }));
                        }
                    }
                }
                Self::Done(..) => panic!("poll after future complete"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl<'a, O: Unpin, R, S> TakeReader<'a, R> for PayloadParse<'a, O, R, S> {
    impl_take_reader! {}

    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Waiting(reader, opts, ..) => (reader, opts),
            Self::Seek(opts, _, _, seek) => (&mut seek.reader, opts),
            Self::Done(reader, opts) => (reader, opts),
            Self::Empty => unreachable!(),
        }
    }
}

impl<O: Offset + Unpin> Payload<O> {
    pub fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> PayloadParse<'a, O, R> {
        let offset = reader.offset();
        let len = reader.remaining_size();
        PayloadParse::Waiting(reader, options, offset, len)
    }
}

impl<O: Offset + Unpin, S> Payload<O, S> {
    pub fn create_fut_from_len<'a, R: PollReader<O> + Unpin>(
        len: S,
        reader: R,
        options: &'a ParseOptions,
    ) -> PayloadParse<'a, O, R, S> {
        let offset = reader.offset();
        PayloadParse::Waiting(reader, options, offset, len)
    }
}
