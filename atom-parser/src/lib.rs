#![feature(array_try_map)]

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AtomSize {
    #[cfg(not(feature = "extended_sized_atoms"))]
    size: u32,
    #[cfg(feature = "extended_sized_atoms")]
    size: u64,
}

impl AtomSize {
    pub fn until_eof(&self) -> bool {
        self.size == 0
    }

    const MIN_ATOM_SIZE_32: u32 = 8;
    #[cfg(feature = "extended_sized_atoms")]
    const MIN_ATOM_SIZE_64: u64 = 16;

    fn parse<T: Reader>(size: u32, reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        #[cfg(feature = "extended_sized_atoms")]
        {
            if !matches!(size, 0 | 1) && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            let size = if size == 1 {
                let extended_size = u64::parse(reader, options)?;
                if extended_size < Self::MIN_ATOM_SIZE_64 {
                    return Err(ParseError::AtomSizeTooSmall)
                }
                extended_size - Self::MIN_ATOM_SIZE_64
            } else {
                (size - Self::MIN_ATOM_SIZE_32) as u64
            };

            Ok(Self {
                size
            })
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            if size == 1 {
                return Err(ParseError::AtomSizeUnsupported);
            }

            if size != 0 && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            Ok(Self {
                size: size - Self::MIN_ATOM_SIZE_32
            })
        }
    }

    async fn parse_async<T: AsyncReader>(size: u32, reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        #[cfg(feature = "extended_sized_atoms")]
        {
            if !matches!(size, 0 | 1) && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            let size = if size == 1 {
                let extended_size = u64::parse_async(reader, options).await?;
                if extended_size < Self::MIN_ATOM_SIZE_64 {
                    return Err(ParseError::AtomSizeTooSmall)
                }
                extended_size - Self::MIN_ATOM_SIZE_64
            } else {
                (size - Self::MIN_ATOM_SIZE_32) as u64
            };

            Ok(Self {
                size
            })
        }
        #[cfg(not(feature = "extended_sized_atoms"))]
        {
            if size == 1 {
                return Err(ParseError::AtomSizeUnsupported);
            }

            if size != 0 && size < Self::MIN_ATOM_SIZE_32 {
                return Err(ParseError::AtomSizeTooSmall)
            }

            Ok(Self {
                size: size - Self::MIN_ATOM_SIZE_32
            })
        }
    }
}

type IoError = std::io::Error;

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Atom Size is too small")]
    AtomSizeTooSmall,
    #[cfg(not(feature = "extended_sized_atoms"))]
    #[error("extended atom sizes (64 bytes) is not supported")]
    AtomSizeUnsupported,
    #[error("io error")]
    Io(#[from] IoError)
}

macro_rules! parse_integers {
    ($($i:ty),*,) => {
        $(
            impl Parse for $i {
                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                    let mut buf = [0; core::mem::size_of::<$i>()];
                    reader.read(&mut buf)?;

                    Ok(if matches!(options.endianess, Endianess::Big) {
                        // likely path
                        <$i>::from_be_bytes(buf)
                    } else {
                        <$i>::from_le_bytes(buf)
                    })
                }

                async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                    let mut buf = [0; core::mem::size_of::<$i>()];
                    reader.async_read(&mut buf).await?;

                    Ok(if matches!(options.endianess, Endianess::Big) {
                        // likely path
                        <$i>::from_be_bytes(buf)
                    } else {
                        <$i>::from_le_bytes(buf)
                    })
                }
            }
        )*
    }
}

parse_integers!{
    u8, u16, u32, u64,
    i8, i16, i32, i64,
}

pub trait Parse: Sized {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError>;
    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError>;
}

impl <const N: usize, I: Parse> Parse for [I; N] {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let default: [(); N] = [(); N];
        default.try_map(|_| I::parse(reader, options))
    }

    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        // TODO: above doesnt work for async
        todo!()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[repr(transparent)]
struct FourCC([u8; 4]);

/*
impl FourCC {
    const fn new(fcc: [u8; 4]) -> Self {
        Self(fcc)
    }

    const fn try_from_str(str: &str) -> Result<Self, >
}
*/

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

    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let inner = u32::parse_async(reader, options).await?;
        Ok(Self(inner.to_be_bytes()))
    }
}

#[derive(Debug)]
pub struct AtomHeader {
    pub size: AtomSize,
    pub fcc: FourCC,
}

impl AtomHeader {
    pub fn skip_atom<T: Reader>(&self, reader: &mut T) -> Result<(), IoError> {
        reader.seek(self.size.size as _)
    }

    pub async fn skip_atom_async<T: AsyncReader>(&self, reader: &mut T) -> Result<(), IoError> {
        reader.async_seek(self.size.size as _).await
    }
}

impl Parse for AtomHeader {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = u32::parse(reader, options)?;
        let fcc = FourCC::parse(reader, options)?;
        let size = AtomSize::parse(size, reader, options)?;
        Ok(Self {
            size,
            fcc,
        })
    }

    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = u32::parse_async(reader, options).await?;
        let fcc = FourCC::parse_async(reader, options).await?;
        let size = AtomSize::parse_async(size, reader, options).await?;
        Ok(Self {
            size,
            fcc,
        })
    }
}

trait Atom: Sized {
    const FCC: FourCC;
    // Ok(exact) if known staticly, Err((lower, upper)) if known size range
    // this is the size *not* including the atoms header (e.g 4 + 4 u32, 4 + 4 + 8 u64)
    /*
    const BODY_SIZE: Result<usize, (usize, usize)>;
    fn parse_body() -> Result<Self, ParseError>;
    */
}

trait SwapOffsets {
    fn swap_offsets(&mut self, offset: &mut usize);
}

pub trait Reader: SwapOffsets {
    fn remaining_size(&self) -> usize;
    fn offset(&self) -> usize;
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError>;
    fn seek(&mut self, amt: usize) -> Result<(), IoError>;

    fn seek_remaining(&mut self) -> Result<(), IoError> {
        self.seek(self.remaining_size())
    }
}

impl <'a, S: SwapOffsets> SwapOffsets for &'a mut S {
    fn swap_offsets(&mut self, offset: &mut usize) {
        S::swap_offsets(self, offset)
    }
}

impl <'a, R: Reader> Reader for &'a mut R {
    fn remaining_size(&self) -> usize {
        R::remaining_size(self)
    }

    fn offset(&self) -> usize {
        R::offset(self)
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        R::read(self, bytes)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        R::seek(self, amt)
    }
}

pub trait AsyncReader {
    async fn async_read(&mut self, bytes: &mut [u8]) -> Result<(), IoError>;
    async fn async_seek(&mut self, amt: usize) -> Result<(), IoError>;
    fn offset(&self) -> usize;
    fn remaining_size(&self) -> usize;
    async fn seek_remaining(&mut self) -> Result<(), IoError> {
        self.async_seek(self.remaining_size()).await
    }
}

#[derive(Debug, PartialEq, Eq, Default)]
enum Endianess {
    #[default]
    Big,
    Little,
}

#[derive(Default)]
pub struct ParseOptions {
    endianess: Endianess,
}

pub struct InMemoryReader {
    bytes: Vec<u8>,
    offset: usize,
}

impl InMemoryReader {
    pub fn from_path<P: AsRef<std::path::Path>>(p: P) -> Result<Self, IoError> {
        let bytes = std::fs::read(p)?;
        Ok(Self {
            bytes,
            offset: 0,
        })
    }
}

impl SwapOffsets for InMemoryReader {
    fn swap_offsets(&mut self, offset: &mut usize) {
        core::mem::swap(&mut self.offset, offset)
    }
}

impl Reader for InMemoryReader {
    fn remaining_size(&self) -> usize {
        self.bytes.len().checked_sub(self.offset).unwrap_or_default()
    }

    fn offset(&self) -> usize {
        core::cmp::min(self.offset, self.bytes.len())
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        if self.remaining_size() < bytes.len() {
            return Err(std::io::Error::other("not enough spc"));
        }
        bytes.copy_from_slice(&self.bytes[self.offset..self.offset+bytes.len()]);
        self.offset += bytes.len();
        Ok(())
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        if amt > self.remaining_size() {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.offset += amt;
        Ok(())
    }
}

struct ChildrenIter<'a, R: SwapOffsets, C> {
    _pd: core::marker::PhantomData<C>,
    pub reader: BacktrackReader<TrailingReader<&'a mut R>>,
    opts: &'a ParseOptions,
}

// TODO: async equivalent
impl <'a, R: Reader, C: Parse> Iterator for ChildrenIter<'a, R, C> {
    type Item = Result<C, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining_size() > 0 {
            Some(C::parse(&mut self.reader, self.opts))
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct Children<T> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: usize,
}

struct SizedChildren<S, T> {
    _pd: core::marker::PhantomData<T>,
    len: S,
    offset: usize,
}

impl <S: Parse, I: Parse> Parse for SizedChildren<S, I> {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let len = S::parse(reader, options)?;
        let offset = reader.offset();
        Ok(Self {
            _pd: core::marker::PhantomData,
            len,
            offset,
        })
    }
    
    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let len = S::parse_async(reader, options).await?;
        let offset = reader.offset();
        Ok(Self {
            _pd: core::marker::PhantomData,
            len,
            offset,
        })
    }
}

impl <I: Parse> Parse for Children<I> {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            _pd: core::marker::PhantomData,
            offset,
            size,
        })
    }
    
    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            _pd: core::marker::PhantomData,
            offset,
            size
        })
    }

}

struct BacktrackReader<R: SwapOffsets> {
    reader: R,
    stored_offset: usize,
}

impl <R: SwapOffsets> BacktrackReader<R> {
    fn new(mut reader: R, mut backtrack_to: usize) -> Self {
        reader.swap_offsets(&mut backtrack_to);
        Self {
            reader,
            stored_offset: backtrack_to,
        }
    }
}

impl <R: SwapOffsets> Drop for BacktrackReader<R> {
    fn drop(&mut self) {
        self.reader.swap_offsets(&mut self.stored_offset)
    }
}

impl <S: SwapOffsets> SwapOffsets for BacktrackReader<S> {
    fn swap_offsets(&mut self, offset: &mut usize) {
        self.reader.swap_offsets(offset)
    }
}

impl <R: Reader> Reader for BacktrackReader<R> {
    fn remaining_size(&self) -> usize {
        self.reader.remaining_size()
    }
    fn offset(&self) -> usize {
        self.reader.offset()
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        self.reader.read(bytes)
    }

    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        self.reader.seek(amt)
    }
}

struct TrailingReader<R> {
    reader: R,
    max_offset: usize,
}

impl <R: Reader> TrailingReader<R> {
    fn new(reader: R, max_offset: usize) -> Self {
        Self {
            reader,
            max_offset,
        }
    }
}

impl <S: SwapOffsets> SwapOffsets for TrailingReader<S> {
    fn swap_offsets(&mut self, offset: &mut usize) {
        self.reader.swap_offsets(offset)
    }
}

impl <R: Reader> Reader for TrailingReader<R> {
    fn remaining_size(&self) -> usize {
        self.max_offset.checked_sub(self.reader.offset()).unwrap_or_default()
    }

    fn offset(&self) -> usize {
        self.reader.offset()
    }

    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        if bytes.len() > self.remaining_size() {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.reader.read(bytes)?;
        Ok(())
    }
    fn seek(&mut self, amt: usize) -> Result<(), IoError> {
        if amt > self.remaining_size() {
            return Err(std::io::Error::other("not enough spc"));
        }
        self.reader.seek(amt)?;
        Ok(())
    }
}

#[derive(Debug)]
struct Trailing<T> {
    _pd: core::marker::PhantomData<T>,
    offset: usize,
    size: usize,
}

struct Payload {
    offset: usize,
    size: usize,
}

impl Parse for Payload {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
        })
    }

    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
        })
    }
}

impl <I: Parse> Parse for Trailing<I> {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData
        })
    }

    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let offset = reader.offset();
        let size = reader.remaining_size();
        Ok(Self {
            offset,
            size,
            _pd: core::marker::PhantomData
        })
    }
}

struct TrailingIterator<'a, R: SwapOffsets, T> {
    reader: BacktrackReader<TrailingReader<&'a mut R>>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<T>,
}

impl <'a, T: Parse, R: Reader> Iterator for TrailingIterator<'a, R, T> {
    type Item = Result<T, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining_size() == 0 {
            return None;
        }

        let item = T::parse(&mut self.reader, self.opts);
        Some(item)
    }
}

#[derive(Debug)]
struct DynamicArray<S, I, const ZERO_RELATIVE: bool> {
    size: S,
    offset: usize,
    _pd: core::marker::PhantomData<I>,
}

struct DynamicArrayIter<'a, R: SwapOffsets, S, I, const ZERO_RELATIVE: bool> {
    size: S,
    current: S,
    exhausted: bool,
    //arr: &'a DynamicArray<S, I, ZERO_RELATIVE>,
    pub reader: BacktrackReader<&'a mut R>,
    opts: &'a ParseOptions,
    _pd: core::marker::PhantomData<I>,
}

fn dynamic_array_iter_has_next<const ZERO_RELATIVE: bool, S: ArraySize>(current: &mut S, max_size: &S, exhausted: &mut bool) -> bool {
    let has_next = if ZERO_RELATIVE {
        // inclusive range
        if *exhausted {
            false
        } else {
            *current <= *max_size
        }
    } else {
        // exclusive range
        *current < *max_size
    };

    if !has_next {
        return false;
    }

    if ZERO_RELATIVE && current == max_size {
        *exhausted = true;
    } else {
        *current += S::ONE;
    }
    true
}

impl <'a, R: Reader, S: ArraySize, I: Parse, const ZERO_RELATIVE: bool> Iterator for DynamicArrayIter<'a, R, S, I, ZERO_RELATIVE> {
    type Item = Result<I, ParseError>;
    fn next(&mut self) -> Option<Self::Item> {
        let has_next = dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut self.current, &self.size, &mut self.exhausted);

        if !has_next {
            return None;
        }
        Some(I::parse(&mut self.reader, &self.opts))
    }
}

trait ArraySize: Clone + Copy + core::ops::AddAssign + core::cmp::Ord {
    const ZERO: Self;
    const ONE: Self;
}

macro_rules! impl_array_size {
    ($($i:ty),*,) => {
        $(
            impl ArraySize for $i {
                const ZERO: Self = 0;
                const ONE: Self = 1;
            }
        )*
    }
}

impl_array_size!{
    u16, u32,
}

impl <I: Parse, S: Parse + ArraySize, const ZERO_RELATIVE: bool> Parse for DynamicArray<S, I, ZERO_RELATIVE> {
    fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        let size = S::parse(reader, options)?;
        let offset = reader.offset();
        let mut current = S::ZERO;
        let mut exhausted = false;

        while dynamic_array_iter_has_next::<ZERO_RELATIVE, S>(&mut current, &size, &mut exhausted) {
            let _ = I::parse(reader, options)?;
        }

        Ok(Self {
            size,
            offset,
            _pd: core::marker::PhantomData,
        })
    }

    async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
        todo!()
    }
}

trait FlagsParse<B>: Sized {
    fn try_from_bits(bits: B, options: &ParseOptions) -> Result<Self, ParseError>;
}

trait Flags<B> {
    fn from_bits(bits: B) -> Self;
    fn to_bits(self) -> B;
}

/*
mod atoms {
    use super::*;
    use atom_parser_derive::make_atom;

    make_atom! {
        #[atom("root")]
        struct Root {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(root)]
        enum Children {
            FileType,
            Movie,
        }
    }

    make_atom! {
        #[atom("ftyp")]
        struct FileType {
            major_brand: FourCC,
            minor_version: u32,
            // FIXME: this should have a better syntax
            // like `Item = FourCC` or something
            #[trailing_iterator]
            compatible_brands: FourCC,
        }
    }

    make_atom! {
        #[atom("wide")]
        struct Wide {}
    }
    // FIXME: better syntax for payload
    // maybe just like `..Payload` or smth
    make_atom! {
        #[atom("skip")]
        struct Skip {
            #[trailing_payload]
            free_space: Vec<u8>,
        }
    }
    make_atom! {
        #[atom("free")]
        struct Free {
            #[trailing_payload]
            free_space: Vec<u8>,
        }
    }

    make_atom! {
        #[atom("moov")]
        struct Movie {
            #[children]
            children: (),
        }
    }

    // FIXME: preferrably above & below would be grouped 
    make_atom! {
        #[atom(moov)]
        enum Children {
            MovieHeader,
            Clipping,
            Track,
            Userdata,
            ColorTable,
            CompressedMovie,
        }
    }

    make_atom! {
        #[atom("cmov")]
        struct CompressedMovie {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(cmov)]
        enum Children {
            DataCompression,
            CompressedMovieData,
        }
    }

    make_atom! {
        #[atom("dcom")]
        struct DataCompression {
            compression_algorithm: u32,
        }
    }

    make_atom! {
        #[atom("cmvd")]
        struct CompressedMovieData {
            #[trailing_payload]
            compressed_movie_data: Vec<u8>,
        }
    }

    make_atom! {
        #[atom("rmra")]
        struct ReferenceMovie {
            #[trailing_iterator]
            reference_movie_descriptors: ReferenceMovieDescriptor,
        }
    }

    make_atom! {
        #[atom("rmda")]
        struct ReferenceMovieDescriptor {
            #[children]
            children: (),
        }
    }

    make_atom!{
        #[atom(rmda)]
        enum Children {
            DataReference2,
            CPUSpeed,
            VersionCheck,
            ComponentDetect,
            Quality,
        }
    }

    make_atom! {
        // why is there is 2 fccs for the same layout & repr ????
        #[atom("rdrf")]
        struct DataReference2 {
            verion: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            children: dref::Child,
        }
    }

    make_atom! {
        #[atom("rmdr")]
        struct DataRate {
            flags: [u8; 4],
            data_rate: u32,
        }
    }

    make_atom! {
        #[atom("rmcs")]
        struct CPUSpeed {
            flags: u32,
            cpu_speed: u32,
        }
    }

    make_atom! {
        #[atom("rmvc")]
        struct VersionCheck {
            flags: u32,
            software_package: u32,
            version: u32,
            mask: u32,
            check_type: u16,
        }
    }

    make_atom! {
        struct ComponentDescription {
            component_type: FourCC,
            component_subtype: FourCC,
            component_manufacturer: FourCC,
            component_flags: u32,
            component_flags_mask: u32,
        }
    }

    make_atom! {
        #[atom("rmcd")]
        struct ComponentDetect {
            flags: u32,
            component_description: ComponentDescription,
            minimum_version: u32,
        }
    }

    make_atom! {
        #[atom("rmqu")]
        struct Quality {
            quality: u32,
        }
    }

    make_atom! {
        #[atom("mvhd")]
        struct MovieHeader {
            version: u8,
            flags: [u8; 3],
            creation_time: u32,
            modification_time: u32,
            time_scale: u32,
            duration: u32,
            preferred_rate: u32,
            preferred_volume: u16,
            #[reserved]
            reserved: [u8; 10],
            display_matrix: [[u32; 3]; 3],
            preview_time: u32,
            preview_duration: u32,
            poster_time: u32,
            selection_time: u32,
            selection_duration: u32,
            current_time: u32,
            next_track_id: u32,
        }
    }

    // FIXME: change macro name,
    // as since it modifies the struct (reserved, fields, etc)
    // it can be used for both general atoms and other structs
    //
    // also, if multiple structs could be made at the same time instead of having to make a def
    // for each, that would be preferred
    make_atom! {
        struct Color {
            #[reserved]
            reserved: u16,
            red: u16,
            green: u16,
            blue: u16,
        }
    }

    make_atom! {
        #[atom("ctab")]
        struct ColorTable {
            seed: u32,
            flags: u16,
            #[dynamic_array(size_type = u16, zero_relative = true)]
            color_table: Color,
        }
    }

    // TODO
    make_atom! {
        #[atom("udta")]
        struct Userdata {

        }
    }

    make_atom! {
        #[atom("trak")]
        struct Track {
            #[children]
            children: ()
        }
    }

    make_atom! {
        #[atom(trak)]
        enum Children {
            TrackHeader,
            Clipping,
            TrackMatte,
            Edit,
            TrackReference,
            TrackLoadingSettings,
            TrackInputMap,
            Media,
            Userdata,
        }
    }


    // FIXME: the 1 byte version + 3 bytes flags is pretty common,
    // may want to make something for that
    make_atom! {
        #[atom("tkhd")]
        struct TrackHeader {
            version: u8,
            flags: [u8; 3],
            creation_time: u32,
            modification_time: u32,
            track_id: u32,
            #[reserved]
            reserved: [u8; 4],
            duration: u32,
            #[reserved]
            reserved: [u8; 8],
            layer: u16,
            alternate_group: u16,
            volume: u16,
            #[reserved]
            reserved: [u8; 2],
            matrix: [[u32; 3]; 3],
            track_width: u32,
            track_height: u32,
        }
    }

    make_atom! {
        #[atom("clip")]
        struct Clipping {

        }
    }

    make_atom! {
        #[atom("crgn")]
        struct ClippingRegion {
            // ??? there is no info on this
            // page 44
        }
    }

    make_atom! {
        #[atom("matt")]
        struct TrackMatte {
            
        }
    }

    make_atom! {
        #[atom("kmat")]
        struct CompressedMatte {
            version: u8,
            flags: [u8; 3],
        }
    }

    make_atom! {
        #[atom("edts")]
        struct Edit {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(edts)]
        enum Children {
            EditList,
        }
    }

    make_atom! {
        #[atom("elst")]
        struct EditList {
            version: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            table_entries: EditListEntry,
        }
    }

    make_atom! {
        struct EditListEntry {
            duration: u32,
            media_time: u32,
            media_rate: u32,
        }
    }

    make_atom! {
        #[atom("tref")]
        struct TrackReference {
            #[children]
            children: ()
        }
    }

    make_atom! {
        #[atom(tref)]
        enum Children {
            TimeCode,
            ChapterList,
            Synchonization,
            Transcript,
            NonprimarySource,
            Hint,
        }
    }

    make_atom! {
        #[atom("tmcd")]
        struct TimeCode {
            #[trailing_iterator]
            related_track_ids: u32
        }
    }

    make_atom! {
        #[atom("chap")]
        struct ChapterList {
            #[trailing_iterator]
            related_track_ids: u32
        }
    }

    make_atom! {
        #[atom("sync")]
        struct Synchonization {
            #[trailing_iterator]
            related_track_ids: u32
        }
    }

    make_atom! {
        #[atom("scpt")]
        struct Transcript {
            #[trailing_iterator]
            related_track_ids: u32
        }
    }

    make_atom! {
        #[atom("ssrc")]
        struct NonprimarySource {
            #[trailing_iterator]
            related_track_ids: u32
        }
    }

    make_atom! {
        #[atom("hint")]
        struct Hint {
            #[trailing_iterator]
            related_track_ids: u32
        }
    }

    make_atom! {
        #[atom("load")]
        struct TrackLoadingSettings {
            preload_start_time: u32,
            preload_duration: u32,
            preload_flags: u32,
            default_hints: u32,
        }
    }

    make_atom! {
        #[atom("imap")]
        struct TrackInputMap {

        }
    }

    make_atom! {
        #[atom(b"\0\0in")]
        struct TrackInput {
            id: u32,
            #[reserved]
            reserved: [u8; 2],
            child_count: u16,
            #[reserved]
            reserved: [u8; 4],
        }
    }

    make_atom! {
        #[atom(b"\0\0ty")]
        struct InputType {
            ty: u32,
        }
    }

    make_atom! {
        #[atom("obid")]
        struct ObjectId {
            object_id: u32,
        }
    }

    make_atom! {
        #[atom("mdia")]
        struct Media {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(mdia)]
        enum Children {
            MediaHeader,
            HandlerReference,
            MediaInformation,
            Userdata,
        }
    }

    make_atom! {
        #[atom("mdhd")]
        struct MediaHeader {
            version: u8,
            flags: [u8; 3],
            creation_time: u32,
            modification_time: u32,
            time_scale: u32,
            duration: u32,
            language: u16,
            quality: u16,
        }
    }

    make_atom! {
        #[atom("hdlr")]
        struct HandlerReference {
            version: u8,
            flags: [u8; 3],
            component_type: u32,
            component_subtype: u32,
            #[reserved]
            component_manufacturer: u32,
            #[reserved]
            component_flags: u32,
            #[reserved]
            component_flags_mask: u32,
            // TODO: trailing string for component_name
        }
    }

    make_atom! {
        #[atom("minf")]
        struct MediaInformation {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(minf)]
        enum Children {
            VideoMediaInformationHeader,
            SoundMediaInformationHeader,
            BaseMediaInformationHeader,
            BaseMediaInformation,
            HandlerReference,
            DataInformation,
            SampleTable,
        }
    }

    make_atom! {
        #[atom("vmhd")]
        struct VideoMediaInformationHeader {
            version: u8,
            flags: [u8; 3],
            graphics_mode: u16,
            opcolor: [u16; 3],
        }
    }

    make_atom! {
        #[atom("smhd")]
        struct SoundMediaInformationHeader {
            version: u8,
            flags: [u8; 3],
            balance: u16,
            #[reserved]
            resered: [u8; 2]
        }
    }

    make_atom! {
        #[atom("gmhd")]
        struct BaseMediaInformationHeader {
            // actually just empty...
        }
    }

    make_atom! {
        #[atom("gmin")]
        struct BaseMediaInformation {
            version: u8,
            flags: [u8; 3],
            graphics_mode: u16,
            opcolor: [u16; 3],
            balance: u16,
            #[reserved]
            reserved: [u8; 2]
        }
    }

    make_atom! {
        #[atom("dinf")]
        struct DataInformation {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(dinf)]
        enum Children {
            DataReference,
        }
    }

    make_atom! {
        #[atom("dref")]
        struct DataReference {
            verion: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            children: dref::Child,
        }
    }

    make_atom! {
        #[atom(dref)]
        enum Children {
            MacAlias,
            MacResource,
            Url,
        }
    }

    make_atom! {
        #[atom("alis")]
        struct MacAlias {
            verion: u8,
            flags: [u8; 3],
        }
    }

    make_atom! {
        #[atom("rsrc")]
        struct MacResource {
            verion: u8,
            flags: [u8; 3],
        }
    }

    make_atom! {
        #[atom(b"url\0")]
        struct Url {
            verion: u8,
            flags: [u8; 3],
        }
    }

    make_atom! {
        #[atom("stbl")]
        struct SampleTable {
            #[children]
            children: (),
        }
    }

    make_atom! {
        #[atom(stbl)]
        enum Children {
            SampleDescription,
            TimeToSample,
            SyncSample,
            SampleToChunk,
            SampleSize,
            ChunkOffset,
            // ShadowSync, reserved
        }
    }

    make_atom! {
        struct SampleDescriptionEntry {
            size: u32,
            data_format: FourCC,
            reserved: [u8; 6],
            data_reference_index: u16,
        }
    }

    make_atom! {
        #[atom(stsd)]
        enum Children {
            /*
            // video
            Cinepak,
            Jpeg,
            UncompressedRgb,
            UncompressedYuv,
            Graphics,
            Animation,
            AppleVideo,
            KodakPhoto,
            Mpeg,
            MJpegA,
            MJpegB,
            Sorenson,
            // sound
            UncompressedAudio,
            UncompressedBinaryAudio,
            UncompressedTwosComplementAudio,
            LittleEndian16Audio,
            Mace3,
            Mace6,
            Ima4,
            Float32Audio,
            Float64Audio,
            Int24Audio,
            Int32Audio,
            ULawAudio,
            ALawAudio,
            ADPCMACM2,
            IMAADPCMACM17,
            DVAudio,
            QDesign,
            QDesign2,
            PureVoice,
            Mpeg3CBR,
            Mpeg3CBRVBR,
            // timecode
            Timecode,
            // text
            Text, //TODO: children of text (table 3-4)
            // TODO: hypertext ?
            // stream
            MpegStream,
            // sprite
            // TODO: sprite
            // TODO: this whole thing needs to be gone over and checked
            */
        }
    }

    // TODO: typed flags & bitfields

    make_atom! {
        #[atom(stsd)]
        // TODO: need a better parser for this
        struct Flags {
            //a = 1,
        }
    }



    make_atom! {
        #[atom("stsd")]
        struct SampleDescription {
            version: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sample_description_table: stsd::Child,
        }
    }

    make_atom! {
        struct TimeToSampleTableEntry {
            sample_count: u32,
            sample_duration: u32,
        }
    }

    make_atom! {
        #[atom("stts")]
        struct TimeToSample {
            version: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            time_to_sample_table: TimeToSampleTableEntry,
        }
    }

    make_atom! {
        #[atom("stss")]
        struct SyncSample {
            version: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sync_sample_table: u32,
        }
    }

    make_atom! {
        struct SampleToChunkTableEntry {
            first_chunk: u32,
            samples_per_chunk: u32,
            sample_description_id: u32,
        }
    }

    make_atom! {
        #[atom("stsc")]
        struct SampleToChunk {
            version: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sample_to_chunk_table: SampleToChunkTableEntry,
        }
    }

    make_atom! {
        #[atom("stsz")]
        struct SampleSize {
            version: u8,
            flags: [u8; 3],
            sample_size: u32,
            #[dynamic_array(size_type = u32, zero_relative = false)]
            sample_size_table: u32,
        }
    }

    make_atom! {
        #[atom("stco")]
        struct ChunkOffset {
            version: u8,
            flags: [u8; 3],
            #[dynamic_array(size_type = u32, zero_relative = false)]
            chunk_offset_table: u32,
        }
    }

    make_atom! {
        #[atom("vide")]
        struct VideoSampleDescription {
            // how tf is this structured
        }
    }

    #[test]
    fn ftyp() {
        let mut r = InMemoryReader::from_path("../file_example_MOV_480_700kB.mov").unwrap();
        let opts = Default::default();

        use Parse;

        let root = Root::parse(&mut r, &opts).unwrap();
        let mut root_iter = root.children(&mut r, &opts);

        while let Some(child) = root_iter.next() {
            let r = &mut root_iter.reader;
            match child.unwrap() {
                root::Child::FileType(ftyp) => println!("file type: {ftyp:?}"),
                root::Child::Movie(movie) => {
                    println!("moov: {movie:?}");
                    let mut child_iter = movie.children(r, &opts);

                    while let Some(child) = child_iter.next() {
                        let r = &mut child_iter.reader;
                        match child.unwrap() {
                            moov::Child::MovieHeader(hd) => {
                                println!("header: {hd:?}");
                            }
                            moov::Child::Clipping(clip) => {
                                println!("clip: {clip:?}");
                            }
                            moov::Child::Track(track) => {
                                println!("track: {track:?}");
                                let mut child_iter = track.children(r, &opts);
                                while let Some(child) = child_iter.next() {
                                    let r = &mut child_iter.reader;
                                    match child.unwrap() {
                                        trak::Child::TrackHeader(hdr) => println!("track header: {hdr:?}"),
                                        trak::Child::Clipping(c) => println!("clipping: {c:?}"),
                                        trak::Child::TrackMatte(tm) => println!("track matte: {tm:?}"),
                                        trak::Child::Edit(e) => {
                                            println!("edit: {e:?}");
                                            let mut child_iter = e.children(r, &opts);
                                            while let Some(child) = child_iter.next() {
                                                let r = &mut child_iter.reader;
                                                match child.unwrap() {
                                                    edts::Child::EditList(el) => {
                                                        println!("edit list: {el:?}");
                                                        let list_entires = el.table_entries(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                        println!("edit list entries: {list_entires:?}");
                                                    }
                                                    _ => (),
                                                }
                                            }
                                        }
                                        trak::Child::TrackReference(tref) => println!("track ref: {tref:?}"),
                                        trak::Child::TrackLoadingSettings(tls) => println!("track loading: {tls:?}"),
                                        trak::Child::TrackInputMap(tim) => println!("track input map: {tim:?}"),
                                        trak::Child::Media(m) => {
                                            let mut child_iter = m.children(r, &opts);
                                            while let Some(child) = child_iter.next() {
                                                let r = &mut child_iter.reader;
                                                match child.unwrap() {
                                                    mdia::Child::MediaHeader(h) => println!("media heaader: {h:?}"),
                                                    mdia::Child::HandlerReference(r) => println!("href: {r:?}"),
                                                    mdia::Child::MediaInformation(info) => {
                                                        println!("info: {info:?}");
                                                        let mut child_iter = info.children(r, &opts);
                                                        while let Some(child) = child_iter.next() {
                                                            let r = &mut child_iter.reader;
                                                            match child.unwrap() {
                                                                minf::Child::VideoMediaInformationHeader(vid) => println!("vid: {vid:?}"),
                                                                minf::Child::SoundMediaInformationHeader(snd) => println!("snd: {snd:?}"),
                                                                minf::Child::BaseMediaInformationHeader(base) => println!("base: {base:?}"),
                                                                minf::Child::BaseMediaInformation(base) => println!("base info: {base:?}"),
                                                                minf::Child::HandlerReference(href) => println!("href: {href:?}"),
                                                                minf::Child::DataInformation(dinfo) => {
                                                                    println!("dinfo: {dinfo:?}");
                                                                    let mut child_iter = dinfo.children(r, &opts);
                                                                    while let Some(child) = child_iter.next() {
                                                                        let r = &mut child_iter.reader;
                                                                        match child.unwrap() {
                                                                            dinf::Child::DataReference(dref) => {
                                                                                let mut child_iter = dref.children(r, &opts);
                                                                                while let Some(child) = child_iter.next() {
                                                                                    match child.unwrap() {
                                                                                        dref::Child::MacAlias(alis) => println!("alias: {alis:?}"),
                                                                                        dref::Child::MacResource(rsrc) => println!("r: {rsrc:?}"),
                                                                                        dref::Child::Url(url) => println!("url: {url:?}"),
                                                                                        _ => (),
                                                                                    }
                                                                                }
                                                                            }
                                                                            _ => (),
                                                                        }
                                                                    }
                                                                }
                                                                minf::Child::SampleTable(stable) => {
                                                                    println!("stable: {stable:?}");
                                                                    let mut child_iter = stable.children(r, &opts);
                                                                    while let Some(child) = child_iter.next() {
                                                                        let r = &mut child_iter.reader;
                                                                        match child.unwrap() {
                                                                            stbl::Child::SampleDescription(desc) => {
                                                                                println!("{desc:?}");
                                                                                let mut child_iter = desc.sample_description_table(r, &opts);
                                                                                while let Some(child) = child_iter.next() {
                                                                                    let r = &mut child_iter.reader;
                                                                                    println!("sample desc: {child:?}");
                                                                                }
                                                                            }
                                                                            stbl::Child::TimeToSample(tts) => {
                                                                                println!("tts: {tts:?}");
                                                                                let time_to_sample = tts.time_to_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                println!("tts entries: {time_to_sample:?}");
                                                                            }
                                                                            stbl::Child::SyncSample(sync) => {
                                                                                println!("sync sample: {sync:?}");
                                                                                let sync_samples = sync.sync_sample_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                println!("sync sample entries: {sync_samples:?}");
                                                                            }
                                                                            stbl::Child::SampleToChunk(stc) => {
                                                                                println!("stc: {stc:?}");
                                                                                let sample_to_chunk = stc.sample_to_chunk_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                println!("stc entries: {sample_to_chunk:?}");
                                                                            }
                                                                            stbl::Child::SampleSize(ss) => {
                                                                                println!("ss: {ss:?}");
                                                                                let sample_sizes = ss.sample_size_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                println!("ss entries: {sample_sizes:?}");
                                                                            }
                                                                            stbl::Child::ChunkOffset(co) => {
                                                                                println!("co32: {co:?}");
                                                                                let offsets = co.chunk_offset_table(r, &opts).collect::<Result<Vec<_>, _>>().unwrap();
                                                                                println!("co32 entries: {offsets:?}")
                                                                            }
                                                                            stbl::Child::Unsupported(u) => println!("unsupported stbl entry: {u:?}"),
                                                                        }
                                                                    }
                                                                }
                                                                minf::Child::Unsupported(_) => (),
                                                            }
                                                        }
                                                    }
                                                    mdia::Child::Userdata(u) => println!("udata: {u:?}"),
                                                    mdia::Child::Unsupported(_) => (),
                                                }
                                            }
                                        }
                                        trak::Child::Userdata(udata) => println!("udata: {udata:?}"),
                                        trak::Child::Unsupported(fcc) => println!("unsupported in trak: {fcc:?}"),
                                    }
                                }
                            }
                            moov::Child::Userdata(udta) => {
                                println!("udata: {udta:?}");
                            }
                            moov::Child::ColorTable(ctb) => {
                                println!("color table: {ctb:?}");
                            }
                            moov::Child::Unsupported(missed) => {
                                println!("skipped: {missed:?}");
                            }
                            _ => (),
                        }
                    }
                }
                root::Child::Unsupported(a) => println!("unsupported {a:?}"),
            }
        }
        panic!()
    }
}
*/
