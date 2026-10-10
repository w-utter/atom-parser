use crate::{
    AsyncParse, AsyncSeek, AtomHeader, FourCC, Offset, Parse, ParseError, ParseOptions, Parsed,
    PollReader, Reader, TakeReader, TrailingReader, TryFromIntError, impl_take_reader,
};

// an atom with an unknown FCC but known contents
// - this is used for, e.g sample descriptions for quicktime
pub struct AnonymousAtom<T: Parsed, O = usize> {
    pub fourcc: FourCC,
    pub content: <T as Parsed>::Output<O>,
}

impl<T: Parsed, O> core::fmt::Debug for AnonymousAtom<T, O>
where
    <T as Parsed>::Output<O>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnonymousAtom")
            .field("fourcc", &self.fourcc)
            .field("content", &self.content)
            .finish()
    }
}

impl<T: Parsed> Parsed for AnonymousAtom<T> {
    type Output<O> = AnonymousAtom<T, O>;
}

impl<O: Offset, I: Parse<O>> Parse<O> for AnonymousAtom<I> {
    fn parse<T: Reader<O>>(
        reader: &mut T,
        opts: &ParseOptions,
    ) -> Result<<Self as Parsed>::Output<O>, ParseError> {
        let header = AtomHeader::parse(reader, opts)?;
        let atom_size = header
            .size
            .size()
            .map(|size| size.try_into())
            .transpose()
            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))?
            .unwrap_or(reader.remaining_size());

        let max_offset = reader.offset() + atom_size;

        let mut r = TrailingReader::new(&mut *reader, max_offset);
        let content = <I as Parse<O>>::parse(&mut r, opts)?;

        r.seek_remaining()?;

        Ok(AnonymousAtom {
            fourcc: header.fcc,
            content,
        })
    }
}

pub enum AsyncAnonymousAtomParse<'a, R: PollReader<O> + Unpin, T: AsyncParse<O>, O: Offset + Unpin>
where
    <T as Parsed>::Output<O>: Unpin,
{
    Header(<AtomHeader as AsyncParse<O>>::Fut<'a, R>),
    Content(
        AtomHeader,
        <T as AsyncParse<O>>::Fut<'a, TrailingReader<O, R>>,
    ),
    Seek(
        AtomHeader,
        <T as Parsed>::Output<O>,
        AsyncSeek<O, TrailingReader<O, R>>,
        &'a ParseOptions,
    ),
    Done(R, &'a ParseOptions),
    Empty,
}

impl<'a, R: PollReader<O> + Unpin, T: AsyncParse<O>, O: Offset + Unpin> Future
    for AsyncAnonymousAtomParse<'a, R, T, O>
where
    <T as Parsed>::Output<O>: Unpin,
{
    type Output = Result<AnonymousAtom<T, O>, ParseError>;
    fn poll(
        mut self: ::core::pin::Pin<&mut Self>,
        cx: &mut ::core::task::Context<'_>,
    ) -> ::core::task::Poll<Self::Output> {
        use core::pin::Pin;
        use core::task::Poll;
        loop {
            let this = core::mem::replace(&mut *self, Self::Empty);
            match this {
                Self::Header(mut fut) => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Header(fut);
                        return Poll::Pending;
                    }
                    Poll::Ready(header) => {
                        let (reader, opts) = fut.take_reader();
                        let header = match header {
                            Ok(header) => header,
                            Err(e) => {
                                *self = Self::Done(reader, opts);
                                return Poll::Ready(Err(e));
                            }
                        };

                        let atom_size = match header
                            .size
                            .size()
                            .map(|size| size.try_into())
                            .transpose()
                            .map_err(|_| ParseError::IntegerConversion(TryFromIntError))
                        {
                            Ok(size) => size,
                            Err(e) => {
                                *self = Self::Done(reader, opts);
                                return ::core::task::Poll::Ready(Err(e));
                            }
                        }
                        .unwrap_or(reader.remaining_size());

                        let max_offset = reader.offset() + atom_size;
                        let reader = TrailingReader::new(reader, max_offset);

                        *self = Self::Content(header, T::create_fut(reader, opts));
                    }
                },
                Self::Content(header, mut fut) => match Pin::new(&mut fut).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Content(header, fut);
                        return Poll::Pending;
                    }
                    Poll::Ready(content) => {
                        let (reader, opts) = fut.take_reader();
                        let content = match content {
                            Ok(content) => content,
                            Err(e) => {
                                *self = Self::Done(reader.reader, opts);
                                return Poll::Ready(Err(e));
                            }
                        };

                        let rest = PollReader::remaining_size(&reader);
                        *self = Self::Seek(header, content, AsyncSeek::seek(reader, rest), opts);
                    }
                },
                Self::Seek(header, content, mut seek, opts) => match Pin::new(&mut seek).poll(cx) {
                    Poll::Pending => {
                        *self = Self::Seek(header, content, seek, opts);
                        return Poll::Pending;
                    }
                    Poll::Ready(res) => {
                        *self = Self::Done(seek.reader.reader, opts);
                        let _ = res?;
                        return Poll::Ready(Ok(AnonymousAtom {
                            fourcc: header.fcc,
                            content,
                        }));
                    }
                },
                Self::Done(..) => panic!("polled future after complete"),
                Self::Empty => unreachable!(),
            }
        }
    }
}

impl<'a, R: PollReader<O> + Unpin, T: AsyncParse<O>, O: Offset + Unpin> TakeReader<'a, R>
    for AsyncAnonymousAtomParse<'a, R, T, O>
where
    <T as Parsed>::Output<O>: Unpin,
{
    impl_take_reader! {}
    fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
        match self {
            Self::Header(h) => h.borrow_reader(),
            Self::Content(_, c) => {
                let (reader, opts) = c.borrow_reader();
                (&mut reader.reader, opts)
            }
            Self::Done(reader, opts) => (reader, opts),
            _ => unreachable!(),
        }
    }
}

impl<T: AsyncParse<O>, O: Offset + Unpin> AsyncParse<O> for AnonymousAtom<T>
where
    <T as Parsed>::Output<O>: Unpin,
{
    type Fut<'a, R: PollReader<O> + Unpin> = AsyncAnonymousAtomParse<'a, R, T, O>;

    fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R> {
        AsyncAnonymousAtomParse::Header(AtomHeader::create_fut(reader, options))
    }
}
