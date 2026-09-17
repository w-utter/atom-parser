use crate::{AsyncReader, ParseError, ParseOptions, PollReader, Reader, TakeReader, Offset};

pub trait Parse<O: Offset>: Sized {
    fn parse<T: Reader<O>>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError>;
}

pub trait AsyncParse<O: Offset + Unpin>: Sized {
    type Fut<'a, R: PollReader<O> + Unpin>: Future<Output = Result<Self, ParseError>>
        + TakeReader<'a, R>
        + Unpin;
    fn create_fut<'a, R: PollReader<O> + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R>;

    fn parse_async<'a, 'r, T: AsyncReader<O>>(
        reader: &'r mut T,
        options: &'a ParseOptions,
    ) -> <Self as AsyncParse<O>>::Fut<'a, &'r mut T> {
        <Self as AsyncParse<O>>::create_fut(reader, options)
    }
}
