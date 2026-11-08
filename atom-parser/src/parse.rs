use crate::{AsyncReader, Offset, Options, ParseError, PollReader, Reader, TakeReader};

pub trait Parse<O: Offset>: Sized + Parsed {
    fn parse<T: Reader<O>, P: Options>(
        reader: &mut T,
        options: P,
    ) -> Result<<Self as Parsed>::Output<O>, ParseError>;
}

pub trait AsyncParse<O: Offset + Unpin>: Sized + Parsed
where
    <Self as Parsed>::Output<O>: Unpin,
{
    type Fut<P: Options + Unpin, R: PollReader<O> + Unpin>: Future<Output = Result<<Self as Parsed>::Output<O>, ParseError>>
        + TakeReader<P, R>
        + Unpin;
    fn create_fut<P: Options + Unpin, R: PollReader<O> + Unpin>(
        reader: R,
        options: P,
    ) -> Self::Fut<P, R>;

    fn parse_async<'r, P: Options + Unpin, T: AsyncReader<O>>(
        reader: &'r mut T,
        options: P,
    ) -> <Self as AsyncParse<O>>::Fut<P, &'r mut T> {
        <Self as AsyncParse<O>>::create_fut(reader, options)
    }
}

pub trait Parsed {
    type Output<O>;
}
