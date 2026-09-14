use crate::{Child, ChildList, Flag, FlagList, KRATE, Version, VersionList};

#[derive(Clone)]
pub enum AtomField {
    // 1 byte of version followed by 3 bytes of flags
    // common at the start of some atoms
    //
    // #[full_box]
    // struct Flags {
    //      A = 1 << 0,
    //      B = 1 << 1,
    // }
    //
    // expands into an equivalent
    //
    // #[version]
    // version: u8,
    // #[flags]
    // struct Flags: [u8; 3] {
    //      A = 1 << 0,
    //      B = 1 << 1,
    // }
    FullBox {
        name: syn::Ident,
        flags: Vec<Flag>,
    },

    // inline flag definitions inside the atom, with an optional field name
    //
    // #[flags(u32)]
    // <field_name: > struct Flags {
    //   const A = 1 << 0;
    //   const B = 1 << 1;
    // }
    // etc
    Flags {
        field_name: Option<syn::Ident>,
        name: syn::Ident,
        parse_repr: syn::Type,
        flags: Vec<Flag>,
    },
    // inline definitions of different versions
    //
    // #[versions]
    // enum Version {
    //      #[version(0)]
    //      V1 {
    //         /* verison specific */
    //      },
    // }
    Version {
        versions: Vec<Version>,
    },
    // inline definitions of the atoms children
    //
    // #[children]
    // enum Children {
    //      Wide,
    //      Skip,
    // }
    Children {
        size_ty: Option<syn::Type>,
        children: Vec<Child>,
    },
    // attrs that can be shared with both structs and
    Struct(syn::Field, StructFieldAttr),
}

fn parse_normal_field(
    input: syn::parse::ParseStream,
    attrs: Vec<syn::Attribute>,
) -> syn::parse::Result<syn::Field> {
    Ok(syn::Field {
        attrs,
        ..syn::Field::parse_named(input)?
    })
}

fn is_atom_attr(path: &syn::Path) -> bool {
    path.is_ident("full_box")
        || path.is_ident("flags")
        || path.is_ident("versions")
        || path.is_ident("children")
}

fn is_struct_attr(path: &syn::Path) -> bool {
    path.is_ident("dynamic_array")
        || path.is_ident("reserved")
        || path.is_ident("pascal_string")
        || path.is_ident("null_terminated_string")
        || path.is_ident("trailing_array")
        || path.is_ident("payload")
        || path.is_ident("version")
}

impl syn::parse::Parse for AtomField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;

        enum AttrKind {
            Atom,
            Struct,
        }

        let mut atom_attrs = attrs.iter().enumerate().filter_map(|(i, attr)| {
            let path = attr.path();

            Some(if is_atom_attr(path) {
                (i, AttrKind::Atom)
            } else if is_struct_attr(path) {
                (i, AttrKind::Struct)
            } else {
                return None;
            })
        });

        if atom_attrs.clone().count() > 1 {
            panic!("more than 1 attribute specifying fields")
        }

        let atom_attr = atom_attrs
            .next()
            .map(|(idx, kind)| (attrs.swap_remove(idx), kind));

        Ok(match atom_attr {
            Some((attr, AttrKind::Atom)) => Self::parse_from_syn(input, &attr)?,
            Some((attr, AttrKind::Struct)) => Self::Struct(
                parse_normal_field(input, attrs)?,
                StructFieldAttr::parse_from_syn(&attr)?,
            ),
            None => Self::Struct(parse_normal_field(input, attrs)?, StructFieldAttr::Normal),
        })
    }
}

pub struct AtomFields {
    pub inner: Vec<AtomField>,
}

impl AtomFields {
    pub fn group_inline_definitions(
        fields: &[AtomField],
        mod_name: &syn::Ident,
        version: Option<&syn::LitInt>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let inline_definitions = fields
            .iter()
            .filter_map(|field| field.as_inline_definition(version));

        if inline_definitions.clone().count() == 0 {
            return None;
        }

        Some(quote! {
            pub mod #mod_name {
                use super::*;
                #(#inline_definitions)*
            }
        })
    }
}

impl syn::parse::Parse for AtomFields {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        let _ = syn::braced!(content in input);

        let content = content.parse_terminated(AtomField::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();

        Ok(Self { inner })
    }
}

impl AtomField {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        atom_field_attr: &syn::Attribute,
    ) -> syn::parse::Result<Self> {
        let _: syn::Visibility = input.parse()?;
        let path = atom_field_attr.path();
        Ok(if path.is_ident("full_box") {
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;
            let flags = input.parse::<FlagList>()?.flags;
            Self::FullBox { name, flags }
        } else if path.is_ident("flags") {
            let field_name = input.parse()?;
            let _: syn::Token![:] = input.parse()?;
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;
            let parse_repr = atom_field_attr.parse_args::<syn::Type>()?;
            let flags = input.parse::<FlagList>()?.flags;

            Self::Flags {
                field_name,
                name,
                parse_repr,
                flags,
            }
        } else if path.is_ident("versions") {
            let _: syn::Token![enum] = input.parse()?;
            let _: syn::Ident = input.parse()?;

            let versions = input.parse::<VersionList>()?.inner;
            Self::Version { versions }
        } else if path.is_ident("children") {
            let _: syn::Token![enum] = input.parse()?;
            let _: syn::Ident = input.parse()?;

            let size_ty = atom_field_attr.parse_args::<syn::Type>().ok();

            let children = input.parse::<ChildList>()?.inner;
            Self::Children { size_ty, children }
        } else {
            unreachable!("unknown atom attr")
        })
    }

    pub fn as_field_decl(&self, atom_mod: Option<&syn::Ident>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = atom_mod.map(|name| quote!(#name::));

        Some(match self {
            Self::Struct(f, attr) => return attr.as_field_decl(f),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub children: ::#KRATE::children::SizedChildren<#size_ty, #mod_name Child>
                    }
                } else {
                    quote! {
                        pub children: ::#KRATE::children::Children<#mod_name Child>
                    }
                }
            }
            Self::FullBox { name, .. } => {
                quote! {
                    pub atom_flags: #mod_name #name
                }
            }
            Self::Flags {
                field_name, name, ..
            } => {
                let field_name = field_name
                    .clone()
                    .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                quote! {
                    pub #field_name: #mod_name #name
                }
            }
            Self::Version { .. } => unreachable!(),
        })
    }

    pub fn as_helper_fn(&self, atom_mod: Option<&syn::Ident>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let mod_name = atom_mod.map(|name| quote!(#name::));
        Some(match self {
            Self::Struct(f, attr) => return attr.as_helper_fn(f),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub fn children<'a, R: ::#KRATE::reader::Reader>(&self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::DynamicArrayIter<'a, R, #size_ty, #mod_name Child, false> {
                            ::#KRATE::array::DynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }

                        pub fn children_async<'a, R: ::#KRATE::reader::PollReader + ::#KRATE::reader::Reader + ::core::marker::Unpin>(&self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::AsyncDynamicArrayIter<'a, R, #size_ty, #mod_name Child, false> {
                            ::#KRATE::array::AsyncDynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }
                    }
                } else {
                    quote! {
                        pub fn children<'a, R: ::#KRATE::reader::Reader>(&self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::children::ChildrenIter<'a, R, #mod_name Child> {
                            ::#KRATE::children::ChildrenIter::from_children(&self.children, reader, opts)
                        }

                        pub fn children_async<'a, R: ::#KRATE::reader::PollReader + ::#KRATE::reader::Reader + ::core::marker::Unpin>(&self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::children::AsyncChildrenIter<'a, R, #mod_name Child> {
                            ::#KRATE::children::AsyncChildrenIter::from_children(&self.children, reader, opts)
                        }
                    }
                }
            }
            Self::FullBox { .. } | Self::Flags { .. } => return None,
            Self::Version { .. } => unreachable!(),
        })
    }

    pub fn as_sync_parse(
        &self,
        atom_mod: &syn::Ident,
        version_mod: Option<&syn::Ident>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = match version_mod {
            Some(v) => quote!(#atom_mod::#v),
            None => quote!(#atom_mod),
        };

        Some(match self {
            Self::Struct(field, attr) => return attr.as_sync_parse(field),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        let children = <::#KRATE::children::SizedChildren<#size_ty, #mod_name::Child>>::parse(reader, options)?;
                    }
                } else {
                    quote! {
                        let children = <::#KRATE::children::Children<#mod_name::Child>>::parse(reader, options)?;
                    }
                }
            }
            Self::FullBox { name, .. } => {
                quote! {
                    let atom_flags = {
                        let bytes = <[u8; 3]>::parse(reader, options)?;
                        <#mod_name::#name as ::#KRATE::flags::FlagsParse<[u8; 3]>>::try_from_bits(bytes, options)?
                    };
                }
            }
            Self::Flags {
                name,
                parse_repr,
                field_name,
                ..
            } => {
                quote! {
                    let #field_name = {
                        use ::#KRATE::flags::FlagsParse;
                        let flags = <#parse_repr>::parse(reader, options)?;
                        <#mod_name::#name>::try_from_bits(flags, options)?
                    };
                }
            }
            Self::Version { .. } => unreachable!(),
        })
    }

    pub fn as_collection(&self) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(field, attr) => return attr.as_collection(field),
            Self::Children { .. } => {
                quote! {
                    children,
                }
            }
            Self::FullBox { .. } => {
                quote! {
                    atom_flags,
                }
            }
            Self::Flags { field_name, .. } => {
                let field_name = field_name
                    .clone()
                    .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));

                quote! {
                    #field_name,
                }
            }
            Self::Version { .. } => return None,
        })
    }

    pub fn as_inline_definition(
        &self,
        version: Option<&syn::LitInt>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(_, _) => return None,
            Self::Children { children, .. } => {
                let variants = children
                    .iter()
                    .filter_map(|child| {
                        if let Some(v) = version {
                            if !child.version_num.is_empty() && !child.version_num.contains(v) {
                                return None;
                            }
                        } else if !child.version_num.is_empty() {
                            panic!("child version but no version identifier");
                        }
                        Some(&child.name)
                    })
                    .collect::<Vec<_>>();

                let krate = &KRATE;
                quote! {
                    #[derive(Debug)]
                    pub enum Child {
                        #(
                            #variants(#variants),
                        )*
                        Unsupported(::#KRATE::fourcc::FourCC),
                    }

                    impl ::#KRATE::parse::Parse for Child {
                        fn parse<T: ::#KRATE::reader::Reader>(reader: &mut T, options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                            let atom = ::#KRATE::atom_header::AtomHeader::parse(reader, options)?;
                            let atom_size = atom.size.size()
                                .map(|size| size.try_into())
                                .transpose().map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError))?
                                .unwrap_or(reader.remaining_size());

                            let max_offset = reader.offset() + atom_size;
                            let mut r = ::#KRATE::reader::TrailingReader::new(&mut*reader, max_offset);
                            Ok(match atom.fcc {
                                #(
                                    <#variants as ::#krate::Atom>::FCC => {
                                        let atom = <#variants as ::#krate::parse::Parse>::parse(&mut r, options)?;
                                        ::#krate::reader::Reader::seek_remaining(&mut r)?;
                                        Child::#variants(atom)
                                    }
                                )*
                                missed => {
                                    ::#krate::reader::Reader::seek_remaining(&mut r)?;
                                    Child::Unsupported(missed)
                                }
                            })
                        }
                    }

                    pub enum AsyncChildParse<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> {
                        AtomHeader(<::#KRATE::atom_header::AtomHeader as ::#KRATE::parse::AsyncParse>::Fut<'a, R>),
                        #(
                            #variants(<#variants as ::#krate::parse::AsyncParse>::Fut<'a, ::#krate::reader::TrailingReader<R>>),
                        )*
                        Seek(Child, ::#KRATE::reader::AsyncSeek<::#KRATE::reader::TrailingReader<R>>, &'a ::#KRATE::parse_options::ParseOptions),
                        Done(::#KRATE::reader::TrailingReader<R>, &'a ::#KRATE::parse_options::ParseOptions),
                        Empty,
                    }

                    impl ::#KRATE::parse::AsyncParse for Child {
                        type Fut<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> = AsyncChildParse<'a, R>;
                        fn create_fut<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin>(reader: R, options: &'a ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'a, R> {
                            AsyncChildParse::AtomHeader(::#KRATE::atom_header::AtomHeader::create_fut(reader, options))
                        }
                    }

                    impl <'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> Future for AsyncChildParse<'a, R> {
                        type Output = ::core::result::Result<Child, ::#KRATE::error::ParseError>;
                        fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                            loop {
                                let mut this = ::core::mem::replace(&mut*self, Self::Empty);
                                match this {
                                    Self::AtomHeader(mut header) => {
                                        match ::core::pin::Pin::new(&mut header).poll(cx) {
                                            ::core::task::Poll::Pending => {
                                                *self = Self::AtomHeader(header);
                                                return ::core::task::Poll::Pending;
                                            }
                                            ::core::task::Poll::Ready(res) => {
                                                let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(header);

                                                let atom = res?;
                                                let atom_size = atom.size.size()
                                                    .map(|size| size.try_into())
                                                    .transpose()
                                                    .map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError))?
                                                    .unwrap_or(reader.remaining_size());

                                                let max_offset = reader.offset() + atom_size;
                                                let reader = ::#KRATE::reader::TrailingReader::new(reader, max_offset);

                                                *self = match atom.fcc {
                                                    #(
                                                        <#variants as ::#krate::Atom>::FCC => {
                                                            Self::#variants(<#variants as ::#krate::parse::AsyncParse>::create_fut(reader, opts))
                                                        }
                                                    )*
                                                    u => {
                                                        let rest = ::#KRATE::reader::PollReader::remaining_size(&reader);
                                                        Self::Seek(Child::Unsupported(u), ::#KRATE::reader::AsyncSeek::seek(reader, rest), opts)
                                                    }
                                                };
                                            }
                                        }
                                    }
                                    #(
                                        Self::#variants(mut fut) => {
                                            match ::core::pin::Pin::new(&mut fut).poll(cx) {
                                                ::core::task::Poll::Pending => {
                                                    *self = Self::#variants (fut);
                                                    return ::core::task::Poll::Pending;
                                                }
                                                ::core::task::Poll::Ready(res) => {
                                                    let child = Child::#variants(res?);
                                                    let (reader, opts) = ::#krate::reader::TakeReader::take_reader(fut);
                                                    let rest = ::#krate::reader::PollReader::remaining_size(&reader);
                                                    *self = Self::Seek(child, ::#krate::reader::AsyncSeek::seek(reader, rest), opts);
                                                }
                                            }
                                        }
                                    )*
                                    Self::Seek(child, mut fut, opts) => {
                                        match ::core::pin::Pin::new(&mut fut).poll(cx) {
                                            ::core::task::Poll::Pending => {
                                                *self = Self::Seek(child, fut, opts);
                                                return ::core::task::Poll::Pending;
                                            }
                                            ::core::task::Poll::Ready(res) => {
                                                let _ = res?;
                                                let reader = fut.reader;
                                                *self = Self::Done(reader, opts);
                                                return ::core::task::Poll::Ready(Ok(child))
                                            }
                                        }
                                    }
                                    Self::Done(..) => panic!("poll after completion"),
                                    Self::Empty => unreachable!(),
                                }
                            }
                        }
                    }

                    impl <'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> AsyncChildParse<'a, R> {
                        pub fn reader(&mut self) -> &mut ::#KRATE::reader::TrailingReader<R> {
                            match self {
                                Self::Done(reader, _) => reader,
                                _ => unreachable!("should not be able to access mid future")
                            }
                        }
                    }

                    impl <'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<'a, R> for AsyncChildParse<'a, R> {
                        fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Done(reader, opts) => (reader.reader, opts),
                                _ => unreachable!("invalid state of AsyncChildParse")
                            }
                        }
                        fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::AtomHeader(fut) => fut.borrow_reader(),
                                #(Self::#variants(fut) => {
                                    let (reader, opts) = fut.borrow_reader();
                                    (&mut reader.reader, opts)
                                })*
                                Self::Seek(_, seek, opts) => (&mut seek.reader.reader, opts),
                                Self::Done(reader, opts) => (&mut reader.reader, opts),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }
                }
            }
            Self::FullBox { name, flags } => {
                let can_elide_flags = FlagList::can_elide_flag_storage(&flags, version);
                let flags_debug = FlagList::debug_impl(flags, name, version, can_elide_flags);

                let repr = syn::parse_quote!(u32);
                let flags_check = FlagList::flags_parse_check(flags, version);
                //let flags_impl = FlagList::flags_trait_impl(name, flags, &repr, version);
                let flags_impl = FlagList::flags_impl(name, flags, &repr, version);
                let flag_storage_elision =
                    can_elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                let bitops_impl = FlagList::flags_bitops_impl(name);
                let async_parse_repr = syn::parse_quote!([u8; 3]);
                let async_parse_impl = FlagList::async_parse_impl(name, &async_parse_repr);

                let to_bits_impl = if can_elide_flags {
                    let expected_vals = flags.iter().filter_map(|flag| {
                        if let Some(v) = version {
                            if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                                return None;
                            }
                        } else if !flag.version_num.is_empty() {
                            panic!("version specified for flags but no version identifier")
                        }
                        Some(&flag.val)
                    });

                    let flag_bits = if expected_vals.clone().count() == 0 {
                        quote!(0)
                    } else {
                        quote!(#((#expected_vals))|*)
                    };

                    quote! {
                        #[cfg(not(feature = "store_unknown_fields"))]
                        {
                            #flag_bits
                        }
                        #[cfg(feature = "store_unknown_fields")]
                        {
                            let [b0, b1, b2] = self.inner;
                            u32::from_be_bytes([b0, b1, b2, 0])
                        }
                    }
                } else {
                    quote! {
                        let [b0, b1, b2] = self.inner;
                        u32::from_be_bytes([b0, b1, b2, 0])
                    }
                };

                quote! {
                    #[derive(Clone, Copy, PartialEq, Eq)]
                    pub struct #name {
                        #flag_storage_elision
                        inner: [u8; 3],
                    }

                    impl #name {
                        #(#flags_impl)*
                        pub fn from_bytes(bytes: [u8; 3]) -> Self {
                            Self {
                                #flag_storage_elision
                                inner: bytes,
                            }
                        }

                        const fn from_bits_(bits: u32) -> Self {
                            let [b0, b1, b2, _] = bits.to_be_bytes();
                            Self {
                                #flag_storage_elision
                                inner: [b0, b1, b2]
                            }
                        }
                        const fn to_bits_(self) -> u32 {
                            #to_bits_impl
                        }
                    }

                    impl ::#KRATE::flags::FlagsParse<[u8; 3]> for #name {
                        fn try_from_bits(bits: [u8; 3], options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                            {
                                let [b0, b1, b2] = bits;
                                let bits = u32::from_be_bytes([b0, b1, b2, 0]);
                                #flags_check
                            }

                            Ok(Self {
                                #flag_storage_elision
                                inner: bits,
                            })
                        }
                    }

                    impl ::#KRATE::flags::Flags<u32> for #name {
                        fn from_bits(bits: u32) -> Self {
                            Self::from_bits_(bits)
                        }

                        fn to_bits(self) -> u32 {
                            self.to_bits_()
                        }
                    }

                    #async_parse_impl

                    #bitops_impl
                    #flags_debug
                }
            }
            Self::Flags {
                name,
                parse_repr,
                flags,
                ..
            } => {
                let can_elide_flags = FlagList::can_elide_flag_storage(&flags, version);
                let flags_debug = FlagList::debug_impl(flags, name, version, can_elide_flags);
                let flags_trait_impl =
                    FlagList::flags_trait_impl(name, flags, parse_repr, version, can_elide_flags);

                let bitops_impl = FlagList::flags_bitops_impl(name);
                let async_parse_impl = FlagList::async_parse_impl(name, parse_repr);

                let flags_impl = FlagList::flags_impl(name, flags, parse_repr, version);
                let flag_storage_elision =
                    can_elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                let to_bits_impl = if can_elide_flags {
                    let expected_vals = flags.iter().filter_map(|flag| {
                        if let Some(v) = version {
                            if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                                return None;
                            }
                        } else if !flag.version_num.is_empty() {
                            panic!("version specified for flags but no version identifier")
                        }
                        Some(&flag.val)
                    });

                    let flag_bits = if expected_vals.clone().count() == 0 {
                        quote!(0)
                    } else {
                        quote!(#((#expected_vals))|*)
                    };

                    quote! {
                        #[cfg(not(feature = "store_unknown_fields"))]
                        {
                            #flag_bits
                        }
                        #[cfg(feature = "store_unknown_fields")]
                        {
                            self.inner
                        }
                    }
                } else {
                    quote!(self.inner)
                };

                quote! {
                    #[derive(Clone, Copy, PartialEq, Eq)]
                    pub struct #name {
                        #flag_storage_elision
                        inner: #parse_repr,
                    }

                    impl #name {
                        #(#flags_impl)*

                        const fn from_bits_(bits: #parse_repr) -> Self {
                            Self {
                                #flag_storage_elision
                                inner: bits,
                            }
                        }

                        const fn to_bits_(self) -> #parse_repr {
                            #to_bits_impl
                        }
                    }

                    #flags_trait_impl
                    #flags_debug
                    #bitops_impl
                    #async_parse_impl

                    impl ::#KRATE::flags::Flags<#parse_repr> for #name {
                        fn from_bits(bits: #parse_repr) -> Self {
                            Self::from_bits_(bits)
                        }

                        fn to_bits(self) -> #parse_repr {
                            self.to_bits_()
                        }
                    }
                }
            }
            Self::Version { .. } => return None,
        })
    }
}

#[derive(Clone)]
pub enum StructFieldAttr {
    // definition of repeated fields `Entry` with length of type `Size`
    //
    // #[dynamic_array(size_type = Size, zero_relative = false)]
    // some_field: Entry,
    //
    DynamicArray {
        zero_relative: syn::LitBool,
        length_ty: syn::Type,
    },
    // declaration of reserved field
    //
    // #[reserved]
    // reserved: [u8; 6],
    Reserved,
    // definition of a length prefixed string
    //
    // #[pascal_string(u32)]
    // some_field: String,
    PascalString {
        length_ty: syn::Type,
    },
    // definition of a null terminated string
    //
    // #[null_terminated_string]
    // some_field: String,
    NullTerminatedString,
    // declaration of a trailing array
    //
    // #[trailing_array]
    // trailing: u32
    TrailingArray,
    // definition of trailing payload at the end of the atom
    //
    // #[payload]
    // some_field: [u8],
    Payload,
    // the field that identifies the version number
    //
    // #[version]
    // version: u8,
    VersionIdentifier,
    // just a normal field
    //
    // field: u8,
    Normal,
}

impl StructFieldAttr {
    pub fn parse_from_syn(attr: &syn::Attribute) -> syn::parse::Result<Self> {
        let path = attr.path();
        Ok(if path.is_ident("dynamic_array") {
            let mut size_ty = None;
            let mut zero_relative = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("size_type") {
                    size_ty = Some(
                        meta.value()
                            .expect("no ty for size_type")
                            .parse::<syn::Type>()
                            .expect("could not parse type of size_type"),
                    );
                    Ok(())
                } else if meta.path.is_ident("zero_relative") {
                    zero_relative = Some(
                        meta.value()
                            .expect("no value for zero_relative")
                            .parse::<syn::LitBool>()
                            .expect("could not parse bool from zero_relative"),
                    );
                    Ok(())
                } else {
                    Err(meta.error("unsupported"))
                }
            })?;
            let length_ty = size_ty.unwrap();
            let zero_relative = zero_relative.unwrap();
            Self::DynamicArray {
                zero_relative,
                length_ty,
            }
        } else if path.is_ident("reserved") {
            Self::Reserved
        } else if path.is_ident("null_terminated_string") {
            Self::NullTerminatedString
        } else if path.is_ident("pascal_string") {
            let length_ty = attr.parse_args::<syn::Type>()?;
            Self::PascalString { length_ty }
        } else if path.is_ident("trailing_array") {
            Self::TrailingArray
        } else if path.is_ident("payload") {
            Self::Payload
        } else if path.is_ident("version") {
            Self::VersionIdentifier
        } else {
            unreachable!("unknown attr");
        })
    }

    pub fn as_sync_parse(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        use quote::quote;
        Some(match self {
            Self::Reserved => {
                quote! {
                    let _reserved = <#ty as ::#KRATE::parse::Parse>::parse(reader, options)?;
                    if options.error_on_used_reserved_fields && (_reserved != unsafe { ::core::mem::zeroed::<#ty>() }) {
                        return Err(::#KRATE::error::ParseError::UsedReservedField);
                    }
                }.into()
            },
            Self::DynamicArray {
                length_ty,
                zero_relative,
            } => {
                quote! {
                    let #name = <::#KRATE::array::DynamicArray::<#length_ty, #ty, #zero_relative> as ::#KRATE::parse::Parse>::parse(reader, options)?;
                }
            }
            Self::TrailingArray => quote! {
                let #name = <::#KRATE::trailing::Trailing::<#ty> as ::#KRATE::parse::Parse>::parse(reader, options)?;
            },
            Self::Payload => quote! {
                let #name = <::#KRATE::payload::Payload as ::#KRATE::parse::Parse>::parse(reader, options)?;
            },
            Self::Normal => {
                quote! {
                    let #name = <#ty as ::#KRATE::parse::Parse>::parse(reader, options)?;
                }.into()
            }
            Self::PascalString {
                length_ty
            } => {
                quote! {
                    let #name = <::#KRATE::string::PascalString::<#length_ty> as ::#KRATE::parse::Parse>::parse(reader, options)?;
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    let #name = <::#KRATE::string::NullTerminatedString as ::#KRATE::parse::Parse>::parse(reader, options)?;
                }
            }
            // builtin parsing in versioned impl
            Self::VersionIdentifier => return None,
        })
    }

    pub fn as_collection(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        use quote::quote;
        Some(match self {
            Self::Reserved | Self::VersionIdentifier => return None,
            Self::Normal
            | Self::DynamicArray { .. }
            | Self::TrailingArray
            | Self::Payload
            | Self::PascalString { .. }
            | Self::NullTerminatedString => quote! {
                #name,
            }
            .into(),
        })
    }

    pub fn as_field_decl(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;

        use quote::quote;
        Some(match self {
            Self::Reserved | Self::VersionIdentifier => return None,
            Self::Normal => {
                quote! {
                    pub #name: #ty
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    pub #name: ::#KRATE::array::DynamicArray<#length_ty, #ty, #zero_relative>
                }
            }
            Self::TrailingArray => {
                quote! {
                    pub #name: ::#KRATE::trailing::Trailing<#ty>
                }
            }
            Self::Payload => {
                quote! {
                    pub #name: ::#KRATE::payload::Payload
                }
            }
            Self::PascalString { length_ty } => {
                quote! {
                    pub #name: ::#KRATE::string::PascalString<#length_ty>
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    pub #name: ::#KRATE::string::NullTerminatedString
                }
            }
        })
    }

    pub fn as_helper_fn(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        let name = name.as_ref().unwrap();

        use quote::quote;
        Some(match self {
            Self::Normal | Self::Reserved | Self::VersionIdentifier => return None,
            Self::TrailingArray => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #name<'a, R: ::#KRATE::reader::Reader>(&self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::trailing::TrailingIterator<'a, R, #ty> {
                        ::#KRATE::trailing::TrailingIterator::from_trailing(&self.#name, reader, opts)
                    }

                    pub fn #async_name<'a, R: ::#KRATE::reader::Reader + ::#KRATE::reader::SwapOffsets + ::#KRATE::reader::PollReader + ::core::marker::Unpin>(&self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::trailing::AsyncTrailingIterator<'a, R, #ty> {
                        ::#KRATE::trailing::AsyncTrailingIterator::from_trailing(&self.#name, reader, opts)
                    }
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #name<'a, R: ::#KRATE::reader::Reader>(&'a self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::DynamicArrayIter<'a, R, #length_ty, #ty, #zero_relative> {
                        ::#KRATE::array::DynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }

                    pub fn #async_name<'a, R: ::#KRATE::reader::Reader + ::#KRATE::reader::SwapOffsets + ::#KRATE::reader::PollReader + ::core::marker::Unpin>(&'a self, reader: &'a mut R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::AsyncDynamicArrayIter<'a, R, #length_ty, #ty, #zero_relative> {
                        ::#KRATE::array::AsyncDynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }
                }
            }
            Self::Payload => return None,
            Self::PascalString { .. } => return None,
            Self::NullTerminatedString => return None,
        })
    }
}

pub fn format_parse_impl(
    name: &syn::Ident,
    sync_parsing: &[proc_macro2::TokenStream],
    field_collection: &[proc_macro2::TokenStream],
    generics: &syn::Generics,
    generic_collection: Option<&proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote::quote! {
        impl #impl_generics ::#KRATE::parse::Parse for #name #ty_generics #where_clause {
            fn parse<T: ::#KRATE::reader::Reader>(reader: &mut T, options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                #(#sync_parsing)*
                Ok(Self {
                    #(#field_collection)*
                    #generic_collection
                })
            }
        }
    }
}

pub fn format_async_statemachine(
    fields: &[AtomField],
    struct_name: &syn::Ident,
    mut generics: syn::Generics,
    atom_mod: Option<&syn::Ident>,
    generic_collection: Option<&proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    use quote::quote;
    let mod_name = atom_mod.map(|name| quote!(#name::));

    let parse_name = quote::format_ident!("Async{}Parse", struct_name);
    let typed_fields = fields
        .iter()
        .enumerate()
        .map(|(i, field)| match field {
            AtomField::FullBox { name, .. } => {
                let ty = quote!(#mod_name #name);
                let name = syn::Ident::new("atom_flags", proc_macro2::Span::call_site());
                (name, ty, true)
            }
            AtomField::Flags {
                field_name, name, ..
            } => {
                let ty = quote!(#mod_name #name);
                let name = field_name
                    .clone()
                    .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                (name, ty, true)
            }
            AtomField::Version { .. } => unreachable!(),
            AtomField::Children { size_ty, .. } => {
                let name = syn::Ident::new("children", proc_macro2::Span::call_site());

                let ty = if let Some(size_ty) = size_ty {
                    quote!(::#KRATE::children::SizedChildren<#size_ty, #mod_name Child>)
                } else {
                    quote!(::#KRATE::children::Children<#mod_name Child>)
                };
                (name, ty, true)
            }
            AtomField::Struct(field, attr) => match attr {
                StructFieldAttr::Normal => {
                    let name = field.ident.clone().unwrap();
                    let ty = &field.ty;

                    (name, quote!(#ty), true)
                }
                StructFieldAttr::VersionIdentifier => unreachable!(),
                StructFieldAttr::Payload => {
                    let name = field.ident.clone().unwrap();
                    (name, quote!(::#KRATE::payload::Payload), true)
                }
                StructFieldAttr::PascalString { length_ty } => {
                    let name = field.ident.clone().unwrap();
                    (
                        name,
                        quote!(::#KRATE::string::PascalString<#length_ty>),
                        true,
                    )
                }
                StructFieldAttr::NullTerminatedString => {
                    let name = field.ident.clone().unwrap();
                    (name, quote!(::#KRATE::string::NullTerminatedString), true)
                }
                StructFieldAttr::Reserved => {
                    let name = quote::format_ident!("__reserved_{i}");
                    let ty = &field.ty;
                    (name, quote!(#ty), false)
                }
                StructFieldAttr::DynamicArray {
                    length_ty,
                    zero_relative,
                } => {
                    let ty = &field.ty;
                    let name = field.ident.clone().unwrap();
                    (
                        name,
                        quote!(::#KRATE::array::DynamicArray<#length_ty, #ty, #zero_relative>),
                        true,
                    )
                }
                StructFieldAttr::TrailingArray => {
                    let ty = &field.ty;
                    let name = field.ident.clone().unwrap();
                    (name, quote!(::#KRATE::trailing::Trailing<#ty>), true)
                }
            },
        })
        .collect::<Vec<_>>();

    let mut state_machine_variants = vec![];
    let mut state_machine_variant_parsing = vec![];
    let mut borrow_reader_variants = vec![];

    for idx in 0..typed_fields.len() {
        let completed = &typed_fields[0..idx];
        let in_progress = &typed_fields[idx];
        let state = quote::format_ident!("S{}", idx);

        let variant_fields = completed.iter().filter_map(|(name, ty, display)| {
            if !display {
                return None;
            }

            Some(quote! {
                #name: #ty,
            })
        });

        let in_progress_fut = {
            let (name, ty, _) = in_progress;
            quote! {
                #name: <#ty as ::#KRATE::parse::AsyncParse>::Fut<'a, R>,
            }
        };

        let variant = quote! {
            #state {
                #(#variant_fields)*
                #in_progress_fut
            }
        };

        let completed_names = completed
            .iter()
            .filter_map(|(name, _, display)| {
                if !display {
                    return None;
                }
                Some(name)
            })
            .collect::<Vec<_>>();
        let name = &in_progress.0;

        let borrow_reader = quote! {
            Self::#state {
                #name, ..
            } => #name.borrow_reader(),
        };

        let on_poll_ready = if idx == typed_fields.len() - 1 {
            let finished = typed_fields.iter().filter_map(|(name, _, display)| {
                if !display {
                    return None;
                }
                Some(name)
            });
            quote! {
                *self = Self::Done(reader, opts);
                return ::core::task::Poll::Ready(Ok(#struct_name {
                    #(#finished,)*
                    #generic_collection
                }));
            }
        } else {
            // get next state
            let (next_name, next_ty, _) = &typed_fields[idx + 1];
            let next_state = quote::format_ident!("S{}", idx + 1);

            let new_completed = typed_fields[..=idx]
                .iter()
                .filter_map(|(name, _, display)| {
                    if !display {
                        return None;
                    }
                    Some(name)
                });

            quote! {
                let #next_name = <#next_ty as ::#KRATE::parse::AsyncParse>::create_fut(reader, opts);
                *self = Self::#next_state {
                    #(#new_completed,)*
                    #next_name
                };
            }
        };

        let check_reserved = if !in_progress.2 {
            let ty = &in_progress.1;
            Some(quote! {
                if opts.error_on_used_reserved_fields && (#name != unsafe { ::core::mem::zeroed::<#ty>() }) {
                    return ::core::task::Poll::Ready(Err(::#KRATE::error::ParseError::UsedReservedField));
                }
            })
        } else {
            None
        };

        let match_fields = completed.iter().filter_map(|(name, _, display)| {
            if !display {
                return None;
            }

            Some(name)
        });

        let on_state = quote! {
            #parse_name::#state {
                #(#match_fields,)*
                mut #name,
            } => {
                match ::core::pin::Pin::new(&mut #name).poll(cx) {
                    ::core::task::Poll::Pending => {
                        *self = Self::#state {
                            #(#completed_names,)*
                            #name,
                        };
                        return ::core::task::Poll::Pending;
                    }
                    ::core::task::Poll::Ready(r) => {
                        let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(#name);
                        let #name = r?;
                        #check_reserved
                        #on_poll_ready
                    }
                }
            }
        };

        state_machine_variants.push(variant);
        state_machine_variant_parsing.push(on_state);
        borrow_reader_variants.push(borrow_reader);
    }

    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds.push(syn::parse_quote!(::core::marker::Unpin));
        }
    }

    let struct_generics = generics.clone();
    let (struct_impl_generics, struct_ty_generics, struct_where_clause) =
        struct_generics.split_for_impl();

    generics.params.push(syn::parse_quote!('a));
    generics
        .params
        .push(syn::parse_quote!(R: ::#KRATE::reader::PollReader + ::core::marker::Unpin));

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let (start, nop_variant, nop_poll, nop_borrow) =
        if let Some((name, ty, _)) = typed_fields.get(0) {
            (
                quote! {
                    #name: <#ty as ::#KRATE::parse::AsyncParse>::create_fut(reader, options)
                },
                None,
                None,
                None,
            )
        } else {
            (
                quote! {
                    nop: (reader, options)
                },
                Some(quote! {
                    S0 { nop: (R, &'a ::#KRATE::parse_options::ParseOptions) },
                }),
                Some(quote! {
                    Self::S0 { nop: (reader, opts) } => {
                        *self = Self::Done(reader, opts);
                        return ::core::task::Poll::Ready(Ok(#struct_name { }))
                    }
                }),
                Some(quote! {
                    Self::S0 { nop: (reader, opts) } => (reader, opts),
                }),
            )
        };

    quote! {
        pub enum #parse_name #generics {
            #(#state_machine_variants,)*
            #nop_variant
            Done(R, &'a ::#KRATE::parse_options::ParseOptions),
            Empty,
        }

        impl #impl_generics Future for #parse_name #type_generics #where_clause {
            type Output = ::core::result::Result<#struct_name #struct_ty_generics, ::#KRATE::error::ParseError>;
            fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                loop {
                    let mut this = ::core::mem::replace(&mut *self, Self::Empty);
                    match this {
                        #(#state_machine_variant_parsing)*
                        #nop_poll
                        Self::Done(..) => panic!("future polled after completion"),
                        Self::Empty => unreachable!(),
                    }
                }
            }
        }

        impl #struct_impl_generics ::#KRATE::parse::AsyncParse for #struct_name #struct_ty_generics #struct_where_clause {
            type Fut<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> = #parse_name #type_generics ;
            fn create_fut<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin>(reader: R, options: &'a ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'a, R> {
                #parse_name ::S0 {
                    #start
                }
            }
        }

        impl #impl_generics ::#KRATE::reader::TakeReader<'a, R> for #parse_name #type_generics #where_clause {
            fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                match self {
                    Self::Done(reader, opts) => return (reader, opts),
                    _ => unreachable!("invalid state"),
                }
            }

            fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                match self {
                    #(#borrow_reader_variants)*
                    #nop_borrow
                    Self::Done(reader, opts) => (reader, opts),
                    Self::Empty => unreachable!(),
                }
            }
        }
    }.into()
}
