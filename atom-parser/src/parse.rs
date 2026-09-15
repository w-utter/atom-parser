use crate::{AsyncReader, ParseError, ParseOptions, PollReader, Reader, TakeReader};

pub trait Parse: Sized {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError>;
}

pub trait AsyncParse: Sized {
    type Fut<'a, R: PollReader + Unpin>: Future<Output = Result<Self, ParseError>>
        + TakeReader<'a, R>
        + Unpin;
    fn create_fut<'a, R: PollReader + Unpin>(
        reader: R,
        options: &'a ParseOptions,
    ) -> Self::Fut<'a, R>;

    fn parse_async<'a, 'r, T: AsyncReader>(
        reader: &'r mut T,
        options: &'a ParseOptions,
    ) -> <Self as AsyncParse>::Fut<'a, &'r mut T> {
        <Self as AsyncParse>::create_fut(reader, options)
    }
}
