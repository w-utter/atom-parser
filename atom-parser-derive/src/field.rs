use crate::{
    Child, ChildList, Conditionals, Flag, FlagList, GroupedConditionals, KRATE, Version,
    VersionList,
};

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
    Conditional(Conditionals),

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
        || path.is_ident("conditional")
}

fn is_struct_attr(path: &syn::Path) -> bool {
    path.is_ident("dynamic_array")
        || path.is_ident("reserved")
        || path.is_ident("expected")
        || path.is_ident("pascal_string")
        || path.is_ident("null_terminated_string")
        || path.is_ident("trailing_array")
        || path.is_ident("payload")
        || path.is_ident("payload_length")
        || path.is_ident("condition")
        || path.is_ident("reuse_condition")
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
        attrs: &[syn::Attribute],
        generics: &syn::Generics,

        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&PayloadLength>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let payload_len = AtomFields::nested_payload_len(payload_len, fields);
        let inline_definitions = fields.iter().filter_map(|field| {
            field.as_inline_definition(
                version,
                attrs,
                generics,
                generic_collection,
                payload_len.as_ref(),
            )
        });

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

    pub fn payload_len(fields: &[AtomField]) -> Option<PayloadLength> {
        let mut tys = fields.iter().filter_map(|field| match &field {
            AtomField::Struct(field, StructFieldAttr::PayloadLength { offset }) => {
                Some((&field.ty, offset))
            }
            _ => None,
        });

        if tys.clone().count() > 1 {
            panic!("more than one payload length specified");
        }

        let ty = tys.next();
        let mut trailing = fields.iter().enumerate().filter(|(_, field)| {
            matches!(
                field,
                AtomField::Children { .. }
                    | AtomField::Struct(
                        _,
                        StructFieldAttr::Payload | StructFieldAttr::TrailingArray
                    )
            )
        });
        let trailing_count = trailing.clone().count();

        if trailing_count > 1 {
            panic!("more than 1 trailing collection")
        }

        let ty = match (ty, trailing.next()) {
            (Some(_), Some((_, AtomField::Children { .. }))) => panic!(
                "length specified with children is unsupported (it is gotten from the atom size)"
            ),
            (_, Some((i, _))) if i != fields.len() - 1 => {
                panic!("trailing field is not the last field declared")
            }
            (Some(_), None) => panic!("trailing length specified but no trailing field"),
            (Some((ty, offset)), Some(_)) => Some(PayloadLength {
                ty: ty.clone(),
                offset: offset.clone(),
            }),
            (None, _) => None,
        };

        ty
    }
    pub fn nested_payload_len(
        existing: Option<&PayloadLength>,
        fields: &[AtomField],
    ) -> Option<PayloadLength> {
        match (existing, Self::payload_len(fields)) {
            (Some(_), Some(new)) => Some(new),
            (Some(existing), None) => Some(existing.clone()),
            (None, Some(new)) => Some(new),
            (None, None) => None,
        }
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
        } else if path.is_ident("conditional") {
            use syn::Meta;
            let name = match &atom_field_attr.meta {
                Meta::Path(_) => None,
                Meta::List(list) => {
                    let parser = |input: syn::parse::ParseStream| Ok(Some(input.parse()?));
                    use syn::parse::Parser;
                    parser.parse2(list.tokens.clone())?
                }
                Meta::NameValue(_) => panic!("name-value pair for conditional is not supported"),
            };

            let cond = Conditionals::parse_from_syn(name, input)?;
            Self::Conditional(cond)
        } else {
            unreachable!("unknown atom attr")
        })
    }

    pub fn as_field_decl(
        &self,
        atom_mod: Option<&syn::Ident>,
        payload_len: Option<&PayloadLength>,
        generics: &syn::Generics,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = atom_mod.map(|name| quote!(#name::));

        Some(match self {
            Self::Struct(f, attr) => return attr.as_field_decl(f, payload_len),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub children: ::#KRATE::children::SizedChildren<__Offset, #size_ty, #mod_name Child>
                    }
                } else {
                    quote! {
                        pub children: ::#KRATE::children::Children<__Offset, #mod_name Child>
                    }
                }
            }
            Self::Conditional(cond) => {
                let exhaustive = cond.is_exhaustive();
                let name = &cond.field_name;
                let ty = match cond.group().unwrap() {
                    GroupedConditionals::Struct { name, .. } => name,
                    GroupedConditionals::Enum { name, .. } => name,
                };

                let mut generics = generics.clone();
                generics.params.push(syn::parse_quote!(__Offset = usize));

                let (_, ty_generics, _) = generics.split_for_impl();

                if !exhaustive {
                    quote! {
                        pub #name: Option<#mod_name #ty #ty_generics>
                    }
                } else {
                    quote! {
                        pub #name: #mod_name #ty #ty_generics
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

    pub fn as_sync_helper_fn(
        &self,
        atom_mod: Option<&syn::Ident>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let mod_name = atom_mod.map(|name| quote!(#name::));
        Some(match self {
            Self::Struct(f, attr) => return attr.as_sync_helper_fn(f),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub fn children<'__parse_options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::DynamicArrayIter<'__parse_options, __Offset, __Reader, #size_ty, #mod_name Child, false> {
                            ::#KRATE::array::DynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }
                    }
                } else {
                    quote! {
                        pub fn children<'__parse_options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::children::ChildrenIter<'__parse_options, __Offset, __Reader, #mod_name Child> {
                            ::#KRATE::children::ChildrenIter::from_children(&self.children, reader, opts)
                        }
                    }
                }
            }
            Self::FullBox { .. } | Self::Flags { .. } | Self::Conditional(..) => return None,
            Self::Version { .. } => unreachable!(),
        })
    }

    pub fn as_async_helper_fn(
        &self,
        atom_mod: Option<&syn::Ident>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let mod_name = atom_mod.map(|name| quote!(#name::));
        Some(match self {
            Self::Struct(f, attr) => return attr.as_async_helper_fn(f),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub fn children_async<'__parse_options, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::AsyncDynamicArrayIter<'__parse_options, __Offset, __Reader, #size_ty, #mod_name Child, false> {
                            ::#KRATE::array::AsyncDynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }
                    }
                } else {
                    quote! {
                        pub fn children_async<'__parse_options, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::children::AsyncChildrenIter<'__parse_options, __Offset, __Reader, #mod_name Child> {
                            ::#KRATE::children::AsyncChildrenIter::from_children(&self.children, reader, opts)
                        }
                    }
                }
            }
            Self::FullBox { .. } | Self::Flags { .. } | Self::Conditional(..) => return None,
            Self::Version { .. } => unreachable!(),
        })
    }

    pub fn as_sync_parse(
        &self,
        atom_mod: &syn::Ident,
        version_mod: Option<&syn::Ident>,
        payload_len: Option<&PayloadLength>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = match version_mod {
            Some(v) => quote!(#atom_mod::#v),
            None => quote!(#atom_mod),
        };

        Some(match self {
            Self::Struct(field, attr) => return attr.as_sync_parse(field, payload_len),
            Self::Children { size_ty, .. } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        let children = <::#KRATE::children::SizedChildren<__Offset, #size_ty, #mod_name::Child>>::parse(reader, opts)?;
                    }
                } else {
                    quote! {
                        let children = <::#KRATE::children::Children<__Offset, #mod_name::Child>>::parse(reader, opts)?;
                    }
                }
            }
            Self::Conditional(cond) => {
                let enum_name =
                    if let GroupedConditionals::Enum { name, .. } = cond.group().unwrap() {
                        Some(quote!(#name ::))
                    } else {
                        None
                    };

                let conditional = cond.format_conditional(
                    |name, fields, exhaustive| {
                        let payload_len = AtomFields::nested_payload_len(payload_len, fields);
                        let sync_parse = fields.iter().filter_map(|f| {
                            f.as_sync_parse(atom_mod, version_mod, payload_len.as_ref())
                        });
                        let collection = fields.iter().filter_map(|f| f.as_collection());

                        let collection = if let Some(e) = enum_name.as_ref() {
                            quote! {
                                #mod_name:: #e #name (#mod_name::#name {
                                    _pd: ::core::marker::PhantomData,
                                    #(#collection)*
                                })
                            }
                        } else {
                            quote! {
                                #mod_name::#name {
                                    _pd: ::core::marker::PhantomData,
                                    #(#collection)*
                                }
                            }
                        };

                        let collection = if !exhaustive {
                            quote! {
                                Some(#collection)
                            }
                        } else {
                            collection
                        };

                        quote! {
                            #(#sync_parse)*
                            #collection
                        }
                    },
                    || quote!(None),
                );

                let name = &cond.field_name;
                quote! {
                    let #name = #conditional ;
                }
            }
            Self::FullBox { name, .. } => {
                quote! {
                    let atom_flags = {
                        let bytes = <[u8; 3]>::parse(reader, opts)?;
                        <#mod_name::#name as ::#KRATE::flags::FlagsParse<[u8; 3]>>::try_from_bits(bytes, opts)?
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
                        let flags = <#parse_repr>::parse(reader, opts)?;
                        <#mod_name::#name>::try_from_bits(flags, opts)?
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
            Self::Conditional(cond) => {
                let name = &cond.field_name;
                quote! {
                    #name,
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
        attrs: &[syn::Attribute],
        generics: &syn::Generics,

        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&PayloadLength>,
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
                    #(#attrs)*
                    pub enum Child<__Offset = usize> {
                        #(
                            #variants(#variants<__Offset>),
                        )*
                        Unsupported(::#KRATE::children::UnknownChild<__Offset>),
                    }

                    impl ::#KRATE::parse::Parsed for Child {
                        type Output<__Offset> = Child<__Offset>;
                    }

                    impl <__Offset: ::#KRATE::reader::Offset> ::#KRATE::parse::Parse<__Offset> for Child {
                        fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>>(reader: &mut __Reader, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                            let atom = ::#KRATE::atom_header::AtomHeader::parse(reader, opts)?;
                            let atom_size = atom.size.size()
                                .map(|size| size.try_into())
                                .transpose().map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError))?
                                .unwrap_or(reader.remaining_size());

                            let max_offset = reader.offset() + atom_size;
                            let mut r = ::#KRATE::reader::TrailingReader::new(&mut*reader, max_offset);
                            Ok(match atom.fcc {
                                #(
                                    <#variants as ::#krate::Atom>::FCC => {
                                        let atom = <#variants as ::#krate::parse::Parse<__Offset>>::parse(&mut r, opts)?;
                                        ::#krate::reader::Reader::seek_remaining(&mut r)?;
                                        Child::#variants(atom)
                                    }
                                )*
                                missed => {
                                    let extent = ::#krate::reader::Reader::extent(&r);
                                    ::#krate::reader::Reader::seek_remaining(&mut r)?;
                                    Child::Unsupported(::#KRATE::children::UnknownChild::new(missed, extent))
                                }
                            })
                        }
                    }

                    pub enum AsyncChildParse<'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> {
                        AtomHeader(<::#KRATE::atom_header::AtomHeader as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<'__parse_options, __Reader>),
                        #(
                            #variants(<#variants as ::#krate::parse::AsyncParse<__Offset>>::Fut<'__parse_options, ::#krate::reader::TrailingReader<__Offset, __Reader>>),
                        )*
                        Seek(Child<__Offset>, ::#KRATE::reader::AsyncSeek<__Offset, ::#KRATE::reader::TrailingReader<__Offset, __Reader>>, &'__parse_options ::#KRATE::parse_options::ParseOptions),
                        Done(::#KRATE::reader::TrailingReader<__Offset, __Reader>, &'__parse_options ::#KRATE::parse_options::ParseOptions),
                        Empty,
                    }

                    impl <__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin> ::#KRATE::parse::AsyncParse<__Offset> for Child {
                        type Fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = AsyncChildParse<'__parse_options, __Offset, __Reader>;
                        fn create_fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'__parse_options, __Reader> {
                            AsyncChildParse::AtomHeader(::#KRATE::atom_header::AtomHeader::create_fut(reader, opts))
                        }
                    }

                    impl <'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> Future for AsyncChildParse<'__parse_options, __Offset, __Reader> {
                        type Output = ::core::result::Result<<Child as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError>;
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

                                                let atom = match res {
                                                    Ok(atom) => atom,
                                                    Err(e) => {
                                                        let max_offset = reader.offset();
                                                        let reader = ::#KRATE::TrailingReader::new(reader, max_offset);
                                                        *self = Self::Done(reader, opts);
                                                        return ::core::task::Poll::Ready(Err(e));
                                                    }
                                                };

                                                let atom_size = match atom.size.size()
                                                    .map(|size| size.try_into())
                                                    .transpose()
                                                    .map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError)) {
                                                        Ok(size) => size,
                                                        Err(e) => {
                                                            let max_offset = reader.offset();
                                                            let reader = ::#KRATE::TrailingReader::new(reader, max_offset);
                                                            *self = Self::Done(reader, opts);
                                                            return ::core::task::Poll::Ready(Err(e));
                                                        }
                                                    }
                                                    .unwrap_or(reader.remaining_size());

                                                let max_offset = reader.offset() + atom_size;
                                                let reader = ::#KRATE::reader::TrailingReader::new(reader, max_offset);

                                                *self = match atom.fcc {
                                                    #(
                                                        <#variants as ::#krate::Atom>::FCC => {
                                                            Self::#variants(<#variants as ::#krate::parse::AsyncParse<__Offset>>::create_fut(reader, opts))
                                                        }
                                                    )*
                                                    u => {
                                                        let extent = #KRATE::reader::PollReader::extent(&reader);
                                                        let rest = ::#KRATE::reader::PollReader::remaining_size(&reader);
                                                        Self::Seek(Child::Unsupported(::#KRATE::children::UnknownChild::new(u, extent)), ::#KRATE::reader::AsyncSeek::seek(reader, rest), opts)
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
                                                    let (reader, opts) = ::#krate::reader::TakeReader::take_reader(fut);
                                                    let child = match res {
                                                        Ok(child) => child,
                                                        Err(e) => {
                                                            *self = Self::Done(reader, opts);
                                                            return ::core::task::Poll::Ready(Err(e));
                                                        }
                                                    };
                                                    let child = Child::#variants(child);
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
                                                let reader = fut.reader;
                                                *self = Self::Done(reader, opts);
                                                let _ = res?;
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

                    impl <'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> AsyncChildParse<'__parse_options, __Offset, __Reader> {
                        pub fn reader(&mut self) -> &mut ::#KRATE::reader::TrailingReader<__Offset, __Reader> {
                            match self {
                                Self::Done(reader, _) => reader,
                                _ => unreachable!("should not be able to access mid future")
                            }
                        }
                    }

                    impl <'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<'__parse_options, __Reader> for AsyncChildParse<'__parse_options, __Offset, __Reader> {
                        fn take_reader(self) -> (__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Done(reader, opts) => (reader.reader, opts),
                                _ => unreachable!("invalid state of AsyncChildParse")
                            }
                        }
                        fn borrow_reader(&mut self) -> (&mut __Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
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
            Self::Conditional(cond) => {
                let nested_defs = cond.as_async_state(generics, generic_collection, payload_len);

                match cond.group().unwrap() {
                    GroupedConditionals::Struct { name, fields } => {
                        let payload_len = AtomFields::nested_payload_len(payload_len, &fields);
                        let helper_fns = crate::conditional::ConditionalVariant::format_helper_fns(
                            &name, &fields, generics,
                        );

                        let fs = fields
                            .iter()
                            .map(|f| match f {
                                AtomField::Struct(field, StructFieldAttr::ReuseCondition) => {
                                    AtomField::Struct(field.clone(), StructFieldAttr::Normal)
                                }
                                r => r.clone(),
                            })
                            .filter_map(|f| f.as_field_decl(None, payload_len.as_ref(), generics));

                        let definitions = fields.iter().filter_map(|field| {
                            field.as_inline_definition(
                                version,
                                attrs,
                                generics,
                                generic_collection,
                                payload_len.as_ref(),
                            )
                        });

                        let mut generics = generics.clone();
                        generics.params.push(syn::parse_quote!(__Offset = usize));
                        let (_, _, where_clause) = generics.split_for_impl();

                        let pd_generics = generics.params.iter().filter_map(|p| match p {
                            syn::GenericParam::Type(t) => Some(&t.ident),
                            _ => None,
                        });

                        quote! {
                            #nested_defs

                            #(#definitions)*

                            #(#attrs)*
                            pub struct #name #generics #where_clause {
                                pub(super) _pd: core::marker::PhantomData<( #(#pd_generics,)* )>,
                                #(#fs,)*
                            }

                            #helper_fns
                        }
                    }
                    GroupedConditionals::Enum { name, variants } => {
                        let vs = variants.iter().map(|v| {
                            let name = &v.variant_name;
                            let mut generics = generics.clone();
                            generics.params.push(syn::parse_quote!(__Offset = usize));

                            let (_, ty_generics, _) = generics.split_for_impl();

                            quote! {
                                #name(#name #ty_generics)
                            }
                        });

                        let variant_defs = variants.iter().map(|v| {
                            let name = &v.variant_name;
                            let payload_len = AtomFields::nested_payload_len(payload_len, &v.fields);
                            let helper_fns = crate::conditional::ConditionalVariant::format_helper_fns(&v.variant_name, &v.fields, generics);
                            let fs = v.fields.iter().map(|f| {
                                match f {
                                    AtomField::Struct(field, StructFieldAttr::ReuseCondition) => AtomField::Struct(field.clone(), StructFieldAttr::Normal),
                                    r => r.clone(),
                                }
                            }).filter_map(|f| f.as_field_decl(None, payload_len.as_ref(), generics));

                            let mut generics = generics.clone();
                            generics.params.push(syn::parse_quote!(__Offset = usize));

                            let (_, ty_generics, where_clause) = generics.split_for_impl();
                            let pd_generics = generics.params.iter().filter_map(|p| {
                                match p {
                                    syn::GenericParam::Type(t) => Some(&t.ident),
                                    _ => None
                                }
                            });

                            quote! {
                                #(#attrs)*
                                pub struct #name #ty_generics #where_clause {
                                    pub(super) _pd: ::core::marker::PhantomData <(( #(#pd_generics,)* ))>,
                                    #(#fs,)*
                                }

                                #helper_fns
                            }
                        });

                        let field_defs = variants.iter().flat_map(|v| {
                            let payload_len =
                                AtomFields::nested_payload_len(payload_len, &v.fields);
                            v.fields.iter().filter_map(move |f| {
                                f.as_inline_definition(
                                    version,
                                    attrs,
                                    generics,
                                    generic_collection,
                                    payload_len.as_ref(),
                                )
                            })
                        });

                        let mut generics = generics.clone();
                        generics.params.push(syn::parse_quote!(__Offset = usize));
                        let (_, _, where_clause) = generics.split_for_impl();
                        quote! {
                            #nested_defs

                            #(#field_defs)*

                            #(#variant_defs)*

                            #(#attrs)*
                            pub enum #name #generics #where_clause {
                                #(#vs,)*
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
                        fn try_from_bits(bits: [u8; 3], opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
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
    // declaration of a field with an expected val
    // the general case of a reserved field
    //
    // #[expected(4)]
    // payload_size: u8,
    //
    // will expand to
    //
    // let payload_size = /* parse impl */;
    // if payload_size != 4 {
    //      return Err(..)
    // }
    Expected {
        val: syn::Expr,
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
    // declaration of the length of the trailing payload
    // at the moment it only supports non-inclusive length
    // e.g, if a headers length includes itself,
    // it does not calculate the offset
    PayloadLength {
        // specifies an offset from the length given, e.g if a headers
        // length includes itself
        //
        // #[payload_length(offset = 8)]
        offset: Option<syn::Expr>,
    },
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
    // specifies that this field is used for a condition
    // (which may be stored if it is reused)
    Condition,
    // specifies that the field of the same name marked as #[condition] is used
    // e.g, the field is stored but not parsed in the conditional
    ReuseCondition,
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
        } else if path.is_ident("expected") {
            let val = attr.parse_args::<syn::Expr>()?;
            Self::Expected { val }
        } else if path.is_ident("null_terminated_string") {
            Self::NullTerminatedString
        } else if path.is_ident("pascal_string") {
            let length_ty = attr.parse_args::<syn::Type>()?;
            Self::PascalString { length_ty }
        } else if path.is_ident("trailing_array") {
            Self::TrailingArray
        } else if path.is_ident("payload") {
            Self::Payload
        } else if path.is_ident("payload_length") {
            use syn::Meta;
            let offset = match &attr.meta {
                Meta::Path(_) => None,
                Meta::List(list) => {
                    let parser = |input: syn::parse::ParseStream| {
                        let name: syn::Path = input.parse()?;
                        if !name.is_ident("offset") {
                            panic!(
                                "only the `offset` meta is supported in the payload_length attr"
                            );
                        }
                        let _: syn::Token![=] = input.parse()?;
                        Ok(Some(input.parse()?))
                    };
                    use syn::parse::Parser;
                    parser.parse2(list.tokens.clone())?
                }
                Meta::NameValue(_) => panic!("name-value pair for conditional is not supported"),
            };

            Self::PayloadLength { offset }
        } else if path.is_ident("version") {
            Self::VersionIdentifier
        } else if path.is_ident("condition") {
            Self::Condition
        } else if path.is_ident("reuse_condition") {
            Self::ReuseCondition
        } else {
            unreachable!("unknown attr");
        })
    }

    pub fn as_sync_parse(
        &self,
        field: &syn::Field,
        payload_len: Option<&PayloadLength>,
    ) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        use quote::quote;
        Some(match self {
            Self::Reserved => {
                quote! {
                    let _reserved = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                    if opts.error_on_used_reserved_fields && (_reserved != unsafe { ::core::mem::zeroed::<#ty>() }) {
                        return Err(::#KRATE::error::ParseError::UsedReservedField);
                    }
                }.into()
            },
            Self::Expected {
                val,
            } => {
                quote! {
                    let _expected = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                    if _expected != #val {
                        return Err(::#KRATE::error::ParseError::UnexpectedValue);
                    }
                }.into()
            }
            Self::DynamicArray {
                length_ty,
                zero_relative,
            } => {
                quote! {
                    let #name = <::#KRATE::array::DynamicArray::<__Offset, #length_ty, #ty, #zero_relative> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                }
            }
            Self::TrailingArray => {
                if let Some(PayloadLength { offset, ..}) = payload_len {
                    let len = if let Some(offset) = offset {
                        quote!(_payload_len.checked_sub(#offset).ok_or(::#KRATE::error::ParseError::AtomSizeTooSmall)?)
                    } else {
                        quote!(_payload_len)
                    };

                    quote! {
                        let #name = ::#KRATE::trailing::Trailing::parse_from_len(#len, reader, opts)?;
                    }
                } else {
                    quote! {
                        let #name = ::#KRATE::trailing::Trailing::parse(reader, opts)?;
                    }
                }
            }
            Self::PayloadLength{..} => {
                quote! {
                    let _payload_len = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                }
            }
            Self::Payload => {
                if let Some(PayloadLength { offset, ..}) = payload_len {
                    let len = if let Some(offset) = offset {
                        quote!(_payload_len.checked_sub(#offset).ok_or(::#KRATE::error::ParseError::AtomSizeTooSmall)?)
                    } else {
                        quote!(_payload_len)
                    };

                    quote! {
                        let #name = ::#KRATE::payload::Payload::parse_from_len(#len, reader, opts)?;
                    }
                } else {
                    quote! {
                        let #name = ::#KRATE::payload::Payload::parse(reader, opts)?;
                    }
                }
            }
            Self::Normal | Self::Condition => {
                quote! {
                    let #name = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                }.into()
            }
            Self::PascalString {
                length_ty
            } => {
                quote! {
                    let #name = <::#KRATE::string::PascalString::<__Offset, #length_ty> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    let #name = <::#KRATE::string::NullTerminatedString<__Offset> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                }
            }
            // builtin parsing in versioned impl
            Self::VersionIdentifier => return None,
            // moved from a field that was already parsed
            Self::ReuseCondition => return None,
        })
    }

    pub fn as_collection(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        use quote::quote;
        Some(match self {
            Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::PayloadLength { .. }
            | Self::Condition => {
                return None;
            }
            Self::Normal
            | Self::DynamicArray { .. }
            | Self::TrailingArray
            | Self::Payload
            | Self::PascalString { .. }
            | Self::ReuseCondition
            | Self::NullTerminatedString => quote! {
                #name,
            }
            .into(),
        })
    }

    pub fn as_field_decl(
        &self,
        field: &syn::Field,
        payload_len: Option<&PayloadLength>,
    ) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;

        use quote::quote;
        Some(match self {
            Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::PayloadLength { .. }
            | Self::Condition
            | Self::ReuseCondition => return None,
            Self::Normal => {
                quote! {
                    pub #name: <#ty as ::#KRATE::parse::Parsed>::Output<__Offset>
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    pub #name: ::#KRATE::array::DynamicArray<__Offset, #length_ty, #ty, #zero_relative>
                }
            }
            Self::TrailingArray => {
                if let Some(PayloadLength { ty: len_ty, .. }) = payload_len {
                    quote! {
                        pub #name: ::#KRATE::trailing::Trailing<__Offset, #ty, #len_ty>
                    }
                } else {
                    quote! {
                        pub #name: ::#KRATE::trailing::Trailing<__Offset, #ty>
                    }
                }
            }
            Self::Payload => {
                if let Some(PayloadLength { ty: len_ty, .. }) = payload_len {
                    quote! {
                        pub #name: ::#KRATE::payload::Payload<__Offset, #len_ty>
                    }
                } else {
                    quote! {
                        pub #name: ::#KRATE::payload::Payload<__Offset>
                    }
                }
            }
            Self::PascalString { length_ty } => {
                quote! {
                    pub #name: ::#KRATE::string::PascalString<__Offset, #length_ty>
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    pub #name: ::#KRATE::string::NullTerminatedString<__Offset>
                }
            }
        })
    }

    pub fn as_sync_helper_fn(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        let name = name.as_ref().unwrap();

        use quote::quote;
        Some(match self {
            Self::Normal
            | Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::PayloadLength { .. }
            | Self::Payload
            | Self::Condition
            | Self::ReuseCondition
            | Self::PascalString { .. }
            | Self::NullTerminatedString => {
                return None;
            }
            Self::TrailingArray => {
                quote! {
                    pub fn #name<'__parse_options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<::#KRATE::trailing::TrailingIterator<'__parse_options, __Offset, __Reader, #ty>, ::#KRATE::error::ParseError> {
                        ::#KRATE::trailing::TrailingIterator::from_trailing(&self.#name, reader, opts)
                    }
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    pub fn #name<'__parse_options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&'__parse_options self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::DynamicArrayIter<'__parse_options, __Offset, __Reader, #length_ty, #ty, #zero_relative> {
                        ::#KRATE::array::DynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }
                }
            }
        })
    }

    pub fn as_async_helper_fn(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        let name = name.as_ref().unwrap();

        use quote::quote;
        Some(match self {
            Self::Normal
            | Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::PayloadLength { .. }
            | Self::Payload
            | Self::Condition
            | Self::ReuseCondition
            | Self::PascalString { .. }
            | Self::NullTerminatedString => {
                return None;
            }
            Self::TrailingArray => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #async_name<'__parse_options, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<::#KRATE::trailing::AsyncTrailingIterator<'__parse_options, __Offset, __Reader, #ty>, ::#KRATE::error::ParseError> {
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
                    pub fn #async_name<'__parse_options, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&'__parse_options self, reader: &'__parse_options mut __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> ::#KRATE::array::AsyncDynamicArrayIter<'__parse_options, __Offset, __Reader, #length_ty, #ty, #zero_relative> {
                        ::#KRATE::array::AsyncDynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }
                }
            }
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
    let mut generics = generics.clone();

    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds
                .push(syn::parse_quote!(::#KRATE::parse::Parse<__Offset>));
        }
    }

    let parsed_generics = generics.clone();

    generics
        .params
        .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset = usize));

    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, parse_generics, _) = parsed_generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ::#KRATE::parse::Parse<__Offset> for #name #parse_generics #where_clause {
            fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>>(reader: &mut __Reader, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                #(#sync_parsing)*
                Ok(#name {
                    #(#field_collection)*
                    _offset: ::core::marker::PhantomData,
                    #generic_collection
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PayloadLength {
    ty: syn::Type,
    offset: Option<syn::Expr>,
}

pub enum FieldKind<'a> {
    PayloadLength,
    Payload,
    Trailing,
    Reserved,
    Expected(&'a syn::Expr),
    Regular,
    Conditional(&'a Conditionals),
    Condition,
}

impl FieldKind<'_> {
    fn as_next_fut(
        &self,
        next_ty: &proc_macro2::TokenStream,
        generics: &syn::Generics,
        mod_name: Option<&proc_macro2::TokenStream>,
        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&PayloadLength>,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        match self {
            Self::Trailing => {
                if let Some(PayloadLength { offset, .. }) = payload_len {
                    let len = if let Some(offset) = offset {
                        quote!(_payload_len.checked_sub(#offset).ok_or(::#KRATE::error::ParseError::AtomSizeTooSmall)?)
                    } else {
                        quote!(_payload_len)
                    };

                    quote!(::#KRATE::trailing::Trailing::create_fut_from_len(#len, reader, opts))
                } else {
                    quote!(::#KRATE::trailing::Trailing::create_fut(reader, opts))
                }
            }
            Self::Payload => {
                if let Some(PayloadLength { offset, .. }) = payload_len {
                    let len = if let Some(offset) = offset {
                        quote!(_payload_len.checked_sub(#offset).ok_or(::#KRATE::error::ParseError::AtomSizeTooSmall)?)
                    } else {
                        quote!(_payload_len)
                    };

                    quote!(::#KRATE::payload::Payload::create_fut_from_len(#len, reader, opts))
                } else {
                    quote!(::#KRATE::payload::Payload::create_fut(reader, opts))
                }
            }
            FieldKind::Conditional(cond) => {
                let enum_name = match cond.group().unwrap() {
                    GroupedConditionals::Enum { name, .. } => {
                        quote::format_ident!("Async{name}Parse")
                    }
                    GroupedConditionals::Struct { name, .. } => {
                        quote::format_ident!("AsyncOptional{name}Parse")
                    }
                };

                cond.format_conditional(
                    |name, fields, exhaustive| {
                        let payload_len = AtomFields::nested_payload_len(payload_len, fields);
                        let typed = prepare_fields_for_statemachine(
                            fields,
                            generics.clone(),
                            mod_name,
                            payload_len.as_ref(),
                        );
                        let parse_name = quote::format_ident!("Async{}Parse", name);
                        if let Some((nested_next_name, nested_next_ty, nested_next_kind)) =
                            typed.get(0)
                        {
                            let fut = nested_next_kind.as_next_fut(
                                nested_next_ty,
                                generics,
                                mod_name,
                                generic_collection,
                                payload_len.as_ref(),
                            );

                            let name = if !exhaustive {
                                quote::format_ident!("Some{}", name)
                            } else {
                                name.clone()
                            };

                            quote! {
                                #mod_name #enum_name:: #name (#mod_name #parse_name::S0 {
                                    #nested_next_name: #fut,
                                })
                            }
                        } else {
                            let name = if !exhaustive {
                                quote::format_ident!("Some{}", name)
                            } else {
                                name.clone()
                            };

                            quote! {
                                #mod_name #enum_name:: #name (#mod_name #parse_name::S0 {
                                    nop: (reader, opts, ::core::marker::PhantomData)
                                })
                            }
                        }
                    },
                    || {
                        quote! {
                            #mod_name #enum_name:: None (reader, opts, ::core::marker::PhantomData)
                        }
                    },
                )
            }
            _ => {
                quote!(<#next_ty as ::#KRATE::parse::AsyncParse<__Offset>>::create_fut(reader, opts))
            }
        }
    }
}

pub fn prepare_fields_for_statemachine<'a>(
    fields: &'a [AtomField],
    mut generics: syn::Generics,
    mod_name: Option<&proc_macro2::TokenStream>,
    payload_len: Option<&PayloadLength>,
) -> Vec<(syn::Ident, proc_macro2::TokenStream, FieldKind<'a>)> {
    let where_clause = generics
        .where_clause
        .get_or_insert_with(|| syn::WhereClause {
            where_token: <syn::Token![where]>::default(),
            predicates: syn::punctuated::Punctuated::new(),
        });

    for param in &generics.params {
        if let syn::GenericParam::Type(ty) = param {
            let name = &ty.ident;
            where_clause.predicates.push(syn::parse_quote!(<#name as ::#KRATE::parse::Parsed>::Output<__Offset>: ::core::marker::Unpin));
        }
    }

    use quote::quote;

    fields
        .iter()
        .enumerate()
        .filter_map(|(i, field)| Some(match field {
            AtomField::FullBox { name, .. } => {
                let ty = quote!(#mod_name #name);
                let name = syn::Ident::new("atom_flags", proc_macro2::Span::call_site());
                (name, ty, FieldKind::Regular)
            }
            AtomField::Flags {
                field_name, name, ..
            } => {
                let ty = quote!(#mod_name #name);
                let name = field_name
                    .clone()
                    .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                (name, ty, FieldKind::Regular)
            }
            AtomField::Version { .. } => unreachable!(),
            AtomField::Children { size_ty, .. } => {
                let name = syn::Ident::new("children", proc_macro2::Span::call_site());

                let ty = if let Some(size_ty) = size_ty {
                    quote!(::#KRATE::children::SizedChildren<__Offset, #size_ty, #mod_name Child>)
                } else {
                    quote!(::#KRATE::children::Children<__Offset, #mod_name Child>)
                };
                (name, ty, FieldKind::Regular)
            }
            AtomField::Conditional(cond) => {
                let name = cond.field_name.clone();
                (name, quote!(ConditionalStorage), FieldKind::Conditional(cond))
            }
            AtomField::Struct(field, attr) => match attr {
                StructFieldAttr::Normal => {
                    let name = field.ident.clone().unwrap();
                    let ty = &field.ty;

                    (name, quote!(#ty), FieldKind::Regular)
                }
                StructFieldAttr::VersionIdentifier => unreachable!(),
                StructFieldAttr::PayloadLength{..} => {
                    let name = quote::format_ident!("_payload_len");
                    let ty = &field.ty;

                    (name, quote!(#ty), FieldKind::PayloadLength)
                }
                StructFieldAttr::Payload => {
                    let name = field.ident.clone().unwrap();
                    let display_ty = if let Some(PayloadLength { ty: len_ty,  ..}) = payload_len {
                        quote!(::#KRATE::payload::Payload<__Offset, #len_ty>)
                    } else {
                        quote!(::#KRATE::payload::Payload)
                    };
                    (name, display_ty, FieldKind::Payload)
                }
                StructFieldAttr::PascalString { length_ty } => {
                    let name = field.ident.clone().unwrap();
                    (
                        name,
                        quote!(::#KRATE::string::PascalString<__Offset, #length_ty>),
                        FieldKind::Regular,
                    )
                }
                StructFieldAttr::NullTerminatedString => {
                    let name = field.ident.clone().unwrap();
                    (
                        name,
                        quote!(::#KRATE::string::NullTerminatedString<__Offset>),
                        FieldKind::Regular,
                    )
                }
                StructFieldAttr::Reserved => {
                    let name = quote::format_ident!("__reserved_{i}");
                    let ty = &field.ty;
                    (name, quote!(#ty), FieldKind::Reserved)
                }
                StructFieldAttr::Expected { val } => {
                    let name = quote::format_ident!("__expected_{i}");
                    let ty = &field.ty;
                    (name, quote!(#ty), FieldKind::Expected(val))
                }
                StructFieldAttr::DynamicArray {
                    length_ty,
                    zero_relative,
                } => {
                    let ty = &field.ty;
                    let name = field.ident.clone().unwrap();
                    (
                        name,
                        quote!(::#KRATE::array::DynamicArray<__Offset, #length_ty, #ty, #zero_relative>),
                        FieldKind::Regular,
                    )
                }
                StructFieldAttr::TrailingArray => {
                    let ty = &field.ty;
                    let name = field.ident.clone().unwrap();
                    let display_ty = if let Some(PayloadLength { ty: len_ty, ..}) = payload_len {
                        quote!(::#KRATE::trailing::Trailing<__Offset, #ty, #len_ty>)
                    } else {
                        quote!(::#KRATE::trailing::Trailing<__Offset, #ty>)
                    };
                    (name, display_ty, FieldKind::Trailing)
                }
                StructFieldAttr::Condition => {
                    let name = field.ident.clone().unwrap();
                    let ty = &field.ty;

                    (name, quote!(#ty), FieldKind::Condition)
                }
                //ignore moving fields around until after parsing is finished
                StructFieldAttr::ReuseCondition => return None,
            },
        }))
        .collect::<Vec<_>>()
}

pub fn format_statemachine_nop_impl(
    typed_fields: &[(syn::Ident, proc_macro2::TokenStream, FieldKind)],
    generics: &syn::Generics,
    mod_name: Option<&proc_macro2::TokenStream>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    payload_len: Option<&PayloadLength>,
    format_state_transition: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> (
    proc_macro2::TokenStream,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
) {
    use quote::quote;
    if let Some((name, ty, kind)) = typed_fields.get(0) {
        let next_fut = kind.as_next_fut(ty, generics, mod_name, generic_collection, payload_len);

        (
            quote! {
                #name: #next_fut
            },
            None,
            None,
            None,
        )
    } else {
        let state_transition = format_state_transition(Default::default());

        let pd_generics = generics.params.iter().filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(&t.ident),
            _ => None,
        });

        (
            quote! {
                nop: (reader, opts, ::core::marker::PhantomData)
            },
            Some(quote! {
                S0 { nop: (__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions, ::core::marker::PhantomData<( __Offset, #(#pd_generics,)* )>) },
            }),
            Some(quote! {
                Self::S0 { nop: (reader, opts, _) } => {
                    #state_transition
                }
            }),
            Some(quote! {
                Self::S0 { nop: (reader, opts, _) } => (reader, opts),
            }),
        )
    }
}

pub fn format_variant_state_change<'a>(
    name: &syn::Ident,
    variant_name: &syn::Ident,
    enum_name: &syn::Ident,
    variant_parse_name: &syn::Ident,
    fields: &[AtomField],
    typed_fields: &[(syn::Ident, proc_macro2::TokenStream, FieldKind)],
    exhaustive: bool,
    generics: &syn::Generics,
    match_fields: impl Iterator<Item = &'a syn::Ident> + Clone,
    mod_name: Option<&proc_macro2::TokenStream>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    root: bool,
    payload_len: Option<&PayloadLength>,
    struct_name: &syn::Ident,
    idx: usize,
    parse_name: &syn::Ident,
    state: &syn::Ident,
    on_poll_ready: &proc_macro2::TokenStream,
    format_conditional_state: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    format_state_transition: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    use quote::quote;
    let variant = variant_name;

    let parent_typed_fields = &typed_fields;

    let payload_len = AtomFields::nested_payload_len(payload_len, fields);

    let typed_fields =
        prepare_fields_for_statemachine(fields, generics.clone(), mod_name, payload_len.as_ref());

    let format_nested_conditional = |toks| {
        let match_fields = match_fields.clone();
        let variant = if !exhaustive {
            quote::format_ident!("Some{}", variant)
        } else {
            variant.clone()
        };

        let enum_parse_name = quote::format_ident!("Async{}Parse", enum_name);

        let parse_mod = if root { None } else { mod_name };

        let toks = quote! {
            #parse_mod #parse_name::#state {
                #(#match_fields,)*
                #name: #mod_name #enum_parse_name::#variant(#toks),
            }
        };
        format_conditional_state(toks)
    };

    let format_state_transition = |toks| {
        let finished = typed_fields.iter().filter_map(|(f, _, kind)| match kind {
            FieldKind::PayloadLength
            | FieldKind::Reserved
            | FieldKind::Expected(_)
            | FieldKind::Condition => None,
            _ => Some(f),
        });

        let parse_variant = if !exhaustive {
            quote::format_ident!("Some{}", variant)
        } else {
            variant.clone()
        };

        let enum_storage_name = quote::format_ident!("Async{}Storage", enum_name);
        let variant_storage_name = quote::format_ident!("Async{}Storage", variant);

        let mut generics = generics.clone();
        generics.params.push(syn::parse_quote!(__Offset));
        let (_, ty_generics, _) = generics.split_for_impl();

        let var = quote! {
            // any finished nested state will be here
            #toks

            let #name = #mod_name #enum_storage_name::#ty_generics::#parse_variant(#mod_name #variant_storage_name {
                #(#finished,)*
                #generic_collection
                _pd: ::core::marker::PhantomData,
            });
        };

        if idx == parent_typed_fields.len() - 1 {
            format_state_transition(var)
        } else {
            let (_, next_fut, state) = format_statemachine_next_state(
                parent_typed_fields,
                idx,
                payload_len.as_ref(),
                parse_name,
                mod_name,
                generic_collection,
                format_conditional_state,
                &generics,
                root,
            );

            quote! {
                #var
                #next_fut
                *self = #state;
            }
        }
    };

    let parse_name = quote::format_ident!("Async{}Parse", variant);
    let (_state_machine_variants, state_machine_variant_parsing, _borrow_reader_variants) =
        format_statemachine_fields(
            fields,
            &typed_fields,
            payload_len.as_ref(),
            struct_name,
            &parse_name,
            &generics,
            generic_collection,
            mod_name,
            &format_nested_conditional,
            &format_state_transition,
            false,
        );

    let vname = &variant;
    let variant = if !exhaustive {
        quote::format_ident!("Some{}", variant)
    } else {
        variant.clone()
    };

    let nop_variant = if typed_fields.is_empty() {
        let storage_name = quote::format_ident!("Async{}Storage", enum_name);
        let variant_storage_name = quote::format_ident!("Async{}Storage", vname);

        let mut generics = generics.clone();
        generics.params.push(syn::parse_quote!(__Offset));
        let (_, ty_generics, _) = generics.split_for_impl();

        Some(quote! {
            #mod_name #parse_name ::S0 { nop: (reader, opts, _) } => {
                let #name = #mod_name #storage_name:: #ty_generics ::#variant(#mod_name #variant_storage_name { _pd: ::core::marker::PhantomData });
                #on_poll_ready
            }
        })
    } else {
        None
    };

    quote! {
        #mod_name #variant_parse_name::#variant(v) => {
            match v {
                #(#state_machine_variant_parsing)*
                #nop_variant
            }
        }
    }
}

fn format_none_variant_state_change(
    exhaustive: bool,
    enum_name: &syn::Ident,
    mod_name: Option<&proc_macro2::TokenStream>,
    variant_parse_name: &syn::Ident,
    name: &syn::Ident,
    on_poll_ready: &proc_macro2::TokenStream,
    ty_generics: &syn::TypeGenerics,
) -> Option<proc_macro2::TokenStream> {
    if !exhaustive {
        let storage_name = quote::format_ident!("Async{}Storage", enum_name);

        Some(
            quote::quote! {
                #mod_name #variant_parse_name::None(reader, opts, _) => {
                    let #name = #mod_name #storage_name::#ty_generics::None;
                    #on_poll_ready
                }
            }
            .into(),
        )
    } else {
        None
    }
}

pub fn format_statemachine_fields(
    fields: &[AtomField],
    typed_fields: &[(syn::Ident, proc_macro2::TokenStream, FieldKind)],
    payload_len: Option<&PayloadLength>,
    struct_name: &syn::Ident,
    parse_name: &syn::Ident,
    generics: &syn::Generics,
    generic_collection: Option<&proc_macro2::TokenStream>,
    mod_name: Option<&proc_macro2::TokenStream>,
    format_conditional_state: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    format_state_transition: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    root: bool,
) -> (
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    use quote::quote;
    let mut state_machine_variants = vec![];
    let mut state_machine_variant_parsing = vec![];
    let mut borrow_reader_variants = vec![];

    let payload_len = AtomFields::nested_payload_len(payload_len, fields);

    for idx in 0..typed_fields.len() {
        let completed = &typed_fields[0..idx];
        let in_progress = &typed_fields[idx];
        let state = quote::format_ident!("S{}", idx);

        let variant_fields = completed.iter().filter_map(|(name, ty, kind)| match kind {
            FieldKind::Reserved | FieldKind::Expected { .. } => None,
            FieldKind::Conditional(cond) => {
                let storage_name = match cond.group().unwrap() {
                    GroupedConditionals::Struct { name, .. } => {
                        quote::format_ident!("AsyncOptional{}Storage", name)
                    }
                    GroupedConditionals::Enum { name, .. } => {
                        quote::format_ident!("Async{}Storage", name)
                    }
                };

                let mut storage_generics = generics.clone();
                storage_generics.params.push(syn::parse_quote!(__Offset));
                let (_, storage_ty_generics, _) = storage_generics.split_for_impl();

                Some(quote! {
                    #name: #mod_name #storage_name #storage_ty_generics,
                })
            }
            _ => Some(quote! {
                #name: <#ty as ::#KRATE::parse::Parsed>::Output<__Offset>,
            }),
        });

        let in_progress_fut = {
            let (name, ty, kind) = in_progress;
            match kind {
                FieldKind::Payload => {
                    if let Some(PayloadLength { ty: len, .. }) = payload_len.as_ref() {
                        quote!(#name: ::#KRATE::payload::PayloadParse<'__parse_options, __Offset, __Reader, #len>)
                    } else {
                        quote!(#name: ::#KRATE::payload::PayloadParse<'__parse_options, __Offset, __Reader>)
                    }
                }
                FieldKind::Trailing => {
                    let AtomField::Struct(field, _) = &fields[idx] else {
                        unreachable!();
                    };
                    let trailing_ty = &field.ty;
                    if let Some(PayloadLength { ty: len, .. }) = payload_len.as_ref() {
                        quote!(#name: ::#KRATE::trailing::TrailingParse<'__parse_options, __Offset, __Reader, #trailing_ty, #len>)
                    } else {
                        quote!(#name: ::#KRATE::trailing::TrailingParse<'__parse_options, __Offset, __Reader, #trailing_ty>)
                    }
                }
                FieldKind::Conditional(cond) => {
                    let field_name = &cond.field_name;
                    let parse_name = match cond.group().unwrap() {
                        GroupedConditionals::Struct { name, .. } => {
                            quote::format_ident!("AsyncOptional{}Parse", name)
                        }
                        GroupedConditionals::Enum { name, .. } => {
                            quote::format_ident!("Async{}Parse", name)
                        }
                    };

                    let mut parse_generics = generics.clone();
                    parse_generics
                        .params
                        .push(syn::parse_quote!('__parse_options));
                    parse_generics.params.push(syn::parse_quote!(__Offset));
                    parse_generics.params.push(syn::parse_quote!(__Reader));
                    let (_, parse_ty_generics, _) = parse_generics.split_for_impl();

                    quote!(#field_name: #mod_name #parse_name #parse_ty_generics)
                }
                _ => {
                    quote! {
                        #name: <#ty as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<'__parse_options, __Reader>,
                    }
                }
            }
        };

        let variant = quote! {
            #state {
                #(#variant_fields)*
                #in_progress_fut
            }
        };

        let name = &in_progress.0;

        let borrow_reader = quote! {
            Self::#state {
                #name, ..
            } => #name.borrow_reader(),
        };

        let on_poll_ready = if idx == typed_fields.len() - 1 {
            format_state_transition(Default::default())
        } else {
            // get next state
            let (_, next_fut, state) = format_statemachine_next_state(
                typed_fields,
                idx,
                payload_len.as_ref(),
                parse_name,
                mod_name,
                generic_collection,
                format_conditional_state,
                &generics,
                root,
            );

            quote! {
                #next_fut
                *self = #state;
            }
        };

        let check_reserved = match in_progress.2 {
            FieldKind::Reserved => {
                let ty = &in_progress.1;
                Some(quote! {
                    if opts.error_on_used_reserved_fields && (#name != unsafe { ::core::mem::zeroed::<#ty>() }) {
                        *self = Self::Done(reader, opts);
                        return ::core::task::Poll::Ready(Err(::#KRATE::error::ParseError::UsedReservedField));
                    }
                })
            }
            FieldKind::Expected(val) => Some(quote! {
                if #name != #val {
                    *self = Self::Done(reader, opts);
                    return ::core::task::Poll::Ready(Err(::#KRATE::error::ParseError::UnexpectedValue));
                }
            }),
            _ => None,
        };

        let match_fields = completed.iter().filter_map(|(name, _, kind)| {
            if matches!(kind, FieldKind::Reserved | FieldKind::Expected(_)) {
                return None;
            }

            Some(name)
        });

        let on_state = if let FieldKind::Conditional(cond) = in_progress.2 {
            let exhaustive = cond.is_exhaustive();

            let sm_parse = match cond.group().unwrap() {
                GroupedConditionals::Struct {
                    name: enum_name,
                    fields,
                } => {
                    let variant_name = &enum_name;
                    let enum_name = quote::format_ident!("Optional{}", enum_name);
                    let variant_parse_name = quote::format_ident!("Async{}Parse", enum_name);

                    let variant = format_variant_state_change(
                        name,
                        variant_name,
                        &enum_name,
                        &variant_parse_name,
                        &fields,
                        &typed_fields,
                        exhaustive,
                        generics,
                        match_fields.clone(),
                        mod_name,
                        generic_collection,
                        root,
                        payload_len.as_ref(),
                        struct_name,
                        idx,
                        parse_name,
                        &state,
                        &on_poll_ready,
                        format_conditional_state,
                        format_state_transition,
                    );

                    let mut generics = generics.clone();
                    generics.params.push(syn::parse_quote!(__Offset));
                    let (_, ty_generics, _) = generics.split_for_impl();
                    let none_variant = format_none_variant_state_change(
                        exhaustive,
                        &enum_name,
                        mod_name,
                        &variant_parse_name,
                        name,
                        &on_poll_ready,
                        &ty_generics,
                    );

                    quote! {
                        match #name {
                            #variant
                            #none_variant
                        }
                    }
                }
                GroupedConditionals::Enum {
                    name: enum_name,
                    variants,
                } => {
                    let variant_parse_name = quote::format_ident!("Async{}Parse", enum_name);

                    let vs = variants.iter().map(|v| {
                        format_variant_state_change(
                            name,
                            &v.variant_name,
                            &enum_name,
                            &variant_parse_name,
                            &v.fields,
                            &typed_fields,
                            exhaustive,
                            generics,
                            match_fields.clone(),
                            mod_name,
                            generic_collection,
                            root,
                            payload_len.as_ref(),
                            struct_name,
                            idx,
                            parse_name,
                            &state,
                            &on_poll_ready,
                            format_conditional_state,
                            format_state_transition,
                        )
                    });

                    let mut generics = generics.clone();
                    generics.params.push(syn::parse_quote!(__Offset));
                    let (_, ty_generics, _) = generics.split_for_impl();
                    let none_variant = format_none_variant_state_change(
                        exhaustive,
                        &enum_name,
                        mod_name,
                        &variant_parse_name,
                        name,
                        &on_poll_ready,
                        &ty_generics,
                    );

                    quote! {
                        match #name {
                            #(#vs)*
                            #none_variant
                        }
                    }
                }
            };

            let mod_name = if root { None } else { mod_name };

            quote! {
                #mod_name #parse_name::#state {
                    #(#match_fields,)*
                    mut #name,
                } => {
                    #sm_parse
                }
            }
        } else {
            let pending = {
                let match_fields = match_fields.clone();
                let mod_name = if root { None } else { mod_name };
                quote! {
                    #mod_name #parse_name::#state {
                        #(#match_fields,)*
                        #name,
                    }
                }
            };
            let pending = format_conditional_state(pending);

            let mod_name = if root { None } else { mod_name };

            quote! {
                #mod_name #parse_name::#state {
                    #(#match_fields,)*
                    mut #name,
                } => {
                    match ::core::pin::Pin::new(&mut #name).poll(cx) {
                        ::core::task::Poll::Pending => {
                            *self = #pending;
                            return ::core::task::Poll::Pending;
                        }
                        ::core::task::Poll::Ready(r) => {
                            let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(#name);
                            let #name = match r {
                                Ok(#name) => #name,
                                Err(e) => {
                                    *self = Self::Done(reader, opts);
                                    return ::core::task::Poll::Ready(Err(e));
                                }
                            };
                            #check_reserved
                            #on_poll_ready
                        }
                    }
                }
            }
        };

        state_machine_variants.push(variant);
        state_machine_variant_parsing.push(on_state);
        borrow_reader_variants.push(borrow_reader);
    }
    (
        state_machine_variants,
        state_machine_variant_parsing,
        borrow_reader_variants,
    )
}

pub fn format_statemachine_next_state(
    typed_fields: &[(syn::Ident, proc_macro2::TokenStream, FieldKind)],
    idx: usize,
    payload_len: Option<&PayloadLength>,
    parse_name: &syn::Ident,
    mod_name: Option<&proc_macro2::TokenStream>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    format_conditional_state: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    generics: &syn::Generics,
    root: bool,
) -> (
    syn::Ident,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    use quote::quote;

    let (next_name, next_ty, next_kind) = &typed_fields[idx + 1];
    let next_state = quote::format_ident!("S{}", idx + 1);
    let new_completed = typed_fields[..=idx].iter().filter_map(|(name, _, kind)| {
        if matches!(kind, FieldKind::Reserved | FieldKind::Expected(_)) {
            return None;
        }
        Some(name)
    });

    let next_fut =
        next_kind.as_next_fut(next_ty, generics, mod_name, generic_collection, payload_len);
    let next_fut = quote! {
        let #next_name = #next_fut;
    };

    let mod_name = if root { None } else { mod_name };

    let state = quote! {
        #mod_name #parse_name::#next_state {
            #(#new_completed,)*
            #next_name,
            #generic_collection
        }
    };
    // inlines the conditional state
    // e.g
    // let is2 = <create_future>;
    // OuterParse::S3 {
    //   os1,
    //   os2,
    //   s3: InnerParse::S2 {
    //      is1,
    //      is2,
    //   },
    // }
    let state = format_conditional_state(state);
    (next_state, next_fut, state)
}

pub fn format_async_statemachine(
    fields: &[AtomField],
    struct_name: &syn::Ident,
    mut generics: syn::Generics,
    atom_mod: Option<&syn::Ident>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    payload_len: Option<&PayloadLength>,
) -> proc_macro2::TokenStream {
    let payload_len = AtomFields::nested_payload_len(payload_len, fields);
    let mod_name = atom_mod.map(|name| quote!(#name::));
    let typed_fields = prepare_fields_for_statemachine(
        fields,
        generics.clone(),
        mod_name.as_ref(),
        payload_len.as_ref(),
    );
    use quote::quote;

    let parse_name = quote::format_ident!("Async{}Parse", struct_name);

    let format_base = |toks| toks;
    let format_state_transition = |toks| {
        let finished = fields.iter().filter_map(|f| match f {
            AtomField::Conditional(cond) => Some(cond.as_async_field_collection(mod_name.as_ref())),
            field => field.as_collection(),
        });

        quote! {
            #toks

            *self = Self::Done(reader, opts);
            return ::core::task::Poll::Ready(Ok(#struct_name {
                #(#finished)*
                #generic_collection
                _offset: ::core::marker::PhantomData,
            }));
        }
    };

    let (state_machine_variants, state_machine_variant_parsing, borrow_reader_variants) =
        format_statemachine_fields(
            fields,
            &typed_fields,
            payload_len.as_ref(),
            struct_name,
            &parse_name,
            &generics,
            generic_collection,
            mod_name.as_ref(),
            &format_base,
            &format_state_transition,
            true,
        );

    let (start, nop_variant, nop_poll, nop_borrow) = format_statemachine_nop_impl(
        &typed_fields,
        &generics,
        mod_name.as_ref(),
        generic_collection,
        payload_len.as_ref(),
        &format_state_transition,
    );

    let where_clause = generics
        .where_clause
        .get_or_insert_with(|| syn::WhereClause {
            where_token: <syn::Token![where]>::default(),
            predicates: syn::punctuated::Punctuated::new(),
        });

    for param in &generics.params {
        if let syn::GenericParam::Type(ty) = param {
            let name = &ty.ident;
            where_clause.predicates.push(syn::parse_quote!(<#name as ::#KRATE::parse::Parsed>::Output<__Offset>: ::core::marker::Unpin));
        }
    }

    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds.push(syn::parse_quote!(::core::marker::Unpin));
            ty.bounds
                .push(syn::parse_quote!(::#KRATE::parse::AsyncParse<__Offset>));
        }
    }

    let mut parse_generics = generics.clone();
    parse_generics
        .params
        .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin));
    let (parse_impl_generics, _, _) = parse_generics.split_for_impl();

    let struct_generics = generics.clone();
    let (_, struct_ty_generics, struct_where_clause) = struct_generics.split_for_impl();

    generics.params.push(syn::parse_quote!('__parse_options));
    generics
        .params
        .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin));
    generics.params.push(
        syn::parse_quote!(__Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin),
    );

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    quote! {
        pub enum #parse_name #generics #where_clause {
            #(#state_machine_variants,)*
            #nop_variant
            Done(__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions),
            Empty,
        }

        impl #impl_generics Future for #parse_name #type_generics #where_clause {
            type Output = ::core::result::Result<<#struct_name #struct_ty_generics as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError>;
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

        impl #parse_impl_generics ::#KRATE::parse::AsyncParse<__Offset> for #struct_name #struct_ty_generics #struct_where_clause {
            type Fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = #parse_name #type_generics ;
            fn create_fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'__parse_options, __Reader> {
                #parse_name ::S0 {
                    #start
                }
            }
        }

        impl #impl_generics ::#KRATE::reader::TakeReader<'__parse_options, __Reader> for #parse_name #type_generics #where_clause {
            fn take_reader(self) -> (__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
                match self {
                    Self::Done(reader, opts) => return (reader, opts),
                    _ => unreachable!("invalid state"),
                }
            }

            fn borrow_reader(&mut self) -> (&mut __Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
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

pub fn format_parsed_ty(name: &syn::Ident, generics: &syn::Generics) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut output_generics = generics.clone();
    output_generics.params.push(syn::parse_quote!(__Offset));
    let (_, output_generics, _) = output_generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ::#KRATE::parse::Parsed for #name #ty_generics #where_clause {
            type Output<__Offset> = #name #output_generics;
        }
    }
    .into()
}
