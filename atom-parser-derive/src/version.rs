use std::collections::{HashSet, HashMap};
use crate::{AtomField, StructFieldAttr};

#[derive(Clone)]
pub struct Version {
    pub version_num: HashSet<syn::LitInt>,
    _version_name: syn::Ident,
    pub fields: Vec<AtomField>,
}

impl syn::parse::Parse for Version {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        if attrs.len() != 1 {
            panic!("incorrect # of attrs (only need #[version(0)])")
        }

        let version_attr = attrs
            .into_iter()
            .next()
            .unwrap();

        if !version_attr.path().is_ident("version") {
            panic!("unknown attr")
        }

        let version_num = VersionList::parse_version_num_from_attr(version_attr)?;
        let version_name = input.parse()?;
        let content;
        syn::braced!(content in input);
        let content = content.parse_terminated(AtomField::parse, syn::Token![,])?;
        let fields = content.into_iter().collect::<Vec<_>>();

        Ok(Self {
            version_num,
            _version_name: version_name,
            fields,
        })
    }
}

pub struct VersionList {
    pub inner: Vec<Version>,
}

impl VersionList {
    pub fn parse_version_num_from_attr(attr: syn::Attribute) -> syn::parse::Result<HashSet<syn::LitInt>> {
        Ok(attr.parse_args_with(syn::punctuated::Punctuated::<syn::LitInt, syn::Token![,]>::parse_terminated)?.into_iter().collect::<HashSet<_>>())
    }
}

impl syn::parse::Parse for VersionList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let content = content.parse_terminated(Version::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();
        Ok(Self {
            inner,
        })

    }
}

pub fn try_expand_into_versioned_fields(fields: Vec<AtomField>, version: Option<syn::LitInt>) -> syn::Result<Result<Versioned, Vec<AtomField>>> {
    let versions = fields.iter().filter(|field| matches!(field, AtomField::Version { .. }));
    let mut version_identifier = fields.iter().enumerate().filter(|(_, field)| matches!(field, AtomField::FullBox { .. } | AtomField::Struct(_, StructFieldAttr::VersionIdentifier)));

    if version_identifier.clone().count() > 1 {
        panic!("more than 1 version identifier")
    }

    match (version_identifier.next(), &version, versions.count()) {
        (None, Some(_), _) => panic!("verion attr but no version identifier"),
        (None, _, vs) if vs > 0 => panic!("verions present but no version identifier"),
        (Some(_), None, v) if v == 0 => panic!("version identifier but no verisons"),
        (_, Some(_), v) if v > 0 => panic!("version attr and versions present"),
        (Some((i, _)), _, _) if i > 0 => panic!("version identifier needs to be declared as the first field"),
        (None, _, _) => return Ok(Err(fields)),
        (_, Some(version_num), _) => {
            let mut map = HashMap::new();

            let mut version_repr = None;
            let mut out_fields = vec![];

            for field in &fields {
                match field {
                    AtomField::Struct(field, StructFieldAttr::VersionIdentifier) => {
                        version_repr = Some(field.ty.clone());
                    }
                    AtomField::Version { .. } => unreachable!(),
                    field => {
                        if matches!(field, AtomField::FullBox { .. }) {
                            let repr = syn::parse_quote!(u8);
                            version_repr = Some(repr);
                        }
                        out_fields.push(field.clone());
                    }
                }
            }

            map.insert(version_num.clone(), out_fields);
            Ok(Ok(Versioned {
                versions: map,
                version_repr: version_repr.unwrap()
            }))
        }
        _ => {
            let mut map = HashMap::new();

            let mut version_independent = vec![];
            let mut version_repr = None;

            for field in &fields {
                match field {
                    AtomField::Struct(field, StructFieldAttr::VersionIdentifier) => {
                        version_repr = Some(field.ty.clone());
                    }
                    AtomField::Version {
                        versions,
                    } => {
                        for version in versions {
                            for v in &version.version_num {
                                if !map.contains_key(v) {
                                    map.insert(v.clone(), version_independent.clone());
                                }
                                let version_specific = map.get_mut(v).unwrap();
                                version_specific.extend(version.fields.clone());
                            }
                        }
                    }
                    field => {
                        if matches!(field, AtomField::FullBox { .. }) {
                            let repr = syn::parse_quote!(u8);
                            version_repr = Some(repr);
                        }
                        for (_, version_specific) in &mut map {
                            version_specific.push(field.clone());
                        }
                        version_independent.push(field.clone());
                    }
                }
            }

            Ok(Ok(Versioned {
                versions: map,
                version_repr: version_repr.unwrap(),
            }))
        }
    }
}

pub struct Versioned {
    pub version_repr: syn::Type,
    pub versions: HashMap<syn::LitInt, Vec<AtomField>>,
}

impl Versioned {
    pub fn version_mod_from_lit(v: &syn::LitInt) -> syn::Ident {
        quote::format_ident!("v{v}")
    }

    pub fn versioned_struct_from_lit(name: &syn::Ident, v: &syn::LitInt) -> syn::Ident {
        quote::format_ident!("{name}V{v}")
    }

    pub fn versioned_enum_variant_from_lit(v: &syn::LitInt) -> syn::Ident {
        quote::format_ident!("V{v}")
    }

    pub fn format_enum_name_from_versioned_struct(name: &syn::Ident) -> syn::Ident {
        quote::format_ident!("{name}Versions")
    }

    pub fn format_versioned_struct(&self, name: &syn::Ident, attrs: &[syn::Attribute], generics: &syn::Generics) -> proc_macro2::TokenStream {
        use quote::quote;

        let mut generics = generics.clone();
        for param in &mut generics.params {
            if let syn::GenericParam::Type(ty) = param {
                ty.bounds.push(syn::parse_quote!(Unpin));
            }
        }

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let enum_name = Self::format_enum_name_from_versioned_struct(name);
        let enum_variants = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);
            quote!(#variant(#version_mod::#versioned_name #ty_generics),)
        });

        let sync_parsing = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);

            quote!(#v => Self::#variant(#version_mod::#versioned_name::parse(reader, options)?),)
        });

        let async_sm_variants = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);
            quote!(#variant(<#version_mod::#versioned_name #ty_generics as AsyncParse>::Fut<'a, R>))
        });

        let async_parse_enum_name = quote::format_ident!("Async{}Parse", enum_name);

        let async_parse_impl = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            quote!(Self::#variant(mut fut) => {
                match core::pin::Pin::new(&mut fut).poll(cx) {
                    core::task::Poll::Pending => {
                        *self = Self::#variant(fut);
                        return core::task::Poll::Pending;
                    }
                    core::task::Poll::Ready(res) => {
                        let (reader, opts) = fut.take_reader();
                        *self = Self::Done(reader, opts);
                        return core::task::Poll::Ready(Ok(#enum_name::#variant(res?)));
                    }
                }
            })
        });

        let borrow_reader_impl = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            quote!(Self::#variant(fut) => fut.borrow_reader(),)
        });

        let async_create_fut = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);

            quote! {
                #v => #async_parse_enum_name::#variant(<#version_mod::#versioned_name #ty_generics>::create_fut(reader, opts)),
            }
        });

        let version_repr = &self.version_repr;

        let mut async_parse_generics = generics.clone();
        async_parse_generics.params.push(syn::parse_quote!('a));
        async_parse_generics.params.push(syn::parse_quote!(R: PollReader + Unpin));

        let (async_impl_generics, async_ty_generics, async_where_clause) = async_parse_generics.split_for_impl();

        quote! {
            #(#attrs)*
            #[derive(Debug)]
            pub enum #enum_name #generics {
                #(#enum_variants)*
                Unknown(#version_repr),
            }

            use super::*;
            impl #impl_generics Parse for #enum_name #ty_generics #where_clause {
                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                    let version_ident = <#version_repr>::parse(reader, options)?;
                    Ok(match version_ident {
                        #(#sync_parsing)*
                        u => Self::Unknown(u),
                    })
                }
            }

            impl #impl_generics #enum_name #ty_generics #where_clause {
                pub fn create_fut_from_version<'a, R: PollReader + Unpin>(version: #version_repr, reader: R, opts: &'a ParseOptions) -> #async_parse_enum_name #async_ty_generics {
                    match version {
                        #(#async_create_fut)*
                        u => {
                            let remaining = reader.remaining_size();
                            let seek = AsyncSeek::seek(reader, remaining);
                            #async_parse_enum_name::Unknown(u, seek, opts)
                        }
                    }
                }
            }

            pub enum #async_parse_enum_name #async_parse_generics {
                #(#async_sm_variants,)*
                Unknown(#version_repr, AsyncSeek<R>, &'a ParseOptions),
                Done(R, &'a ParseOptions),
                Empty,
            }

            impl #impl_generics AsyncParse for #enum_name #ty_generics #where_clause {
                type Fut<'a, R: PollReader + Unpin> = #async_parse_enum_name #async_ty_generics;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    unreachable!("can only be called when version is available")
                }
            }

            impl #async_impl_generics Future for #async_parse_enum_name #async_ty_generics #async_where_clause {
                type Output = Result<#enum_name #ty_generics, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    loop {
                        let mut this = core::mem::replace(&mut *self, Self::Empty);
                        match this {
                            #(#async_parse_impl)*
                            Self::Unknown(version, mut seek, opts) => {
                                match core::pin::Pin::new(&mut seek).poll(cx) {
                                    core::task::Poll::Pending => {
                                        *self = Self::Unknown(version, seek, opts);
                                        return core::task::Poll::Pending;
                                    }
                                    core::task::Poll::Ready(res) => {
                                        let _ = res?;
                                        let reader = seek.reader;
                                        *self = Self::Done(reader, opts);
                                        return core::task::Poll::Ready(Ok(#enum_name::Unknown(version)));
                                    }
                                }
                            }
                            Self::Done(..) => panic!("poll after completion"),
                            Self::Empty => unreachable!(),
                        }
                    }
                }
            }

            impl #async_impl_generics TakeReader<'a, R> for #async_parse_enum_name #async_ty_generics #async_where_clause {
                impl_take_reader!{}
                fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                    match self {
                        #(#borrow_reader_impl)*
                        Self::Unknown(_, seek, opts) => (&mut seek.reader, opts),
                        Self::Done(reader, opts) => (reader, opts),
                        Self::Empty => unreachable!(),
                    }
                }
            }
        }
    }

    pub fn format_async_parse(&self, name: &syn::Ident, generics: &syn::Generics, atom_mod: &syn::Ident) -> proc_macro2::TokenStream {
        use quote::quote;
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let mut async_parse_generics = generics.clone();
        async_parse_generics.params.push(syn::parse_quote!('a));
        async_parse_generics.params.push(syn::parse_quote!(R: PollReader + Unpin));
        let (async_impl_generics, async_ty_generics, async_where_clause) = async_parse_generics.split_for_impl();

        let enum_name = Self::format_enum_name_from_versioned_struct(name);
        let async_parse_enum_name = quote::format_ident!("Asnyc{}Parse", name);
        let parse_repr = &self.version_repr;

        quote! {
            pub enum #async_parse_enum_name #async_parse_generics {
                Version(<#parse_repr as AsyncParse>::Fut<'a, R>),
                VersionSpecific(<#atom_mod::#enum_name #ty_generics as AsyncParse>::Fut<'a, R>),
                Done(R, &'a ParseOptions),
                Empty,
            }

            impl #async_impl_generics Future for #async_parse_enum_name #async_ty_generics #async_where_clause {
                type Output = Result<#name #ty_generics, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    loop {
                        let mut this = core::mem::replace(&mut*self, Self::Empty);
                        match this {
                            Self::Version(mut fut) => {
                                match core::pin::Pin::new(&mut fut).poll(cx) {
                                    core::task::Poll::Pending => {
                                        *self = Self::Version(fut);
                                        return core::task::Poll::Pending;
                                    }
                                    core::task::Poll::Ready(v) => {
                                        let (reader, opts) = fut.take_reader();
                                        *self = Self::VersionSpecific(<#atom_mod::#enum_name>::create_fut_from_version(v?, reader, opts));
                                    }
                                }
                            }
                            Self::VersionSpecific(mut fut) => {
                                match core::pin::Pin::new(&mut fut).poll(cx) {
                                    core::task::Poll::Pending => {
                                        *self = Self::VersionSpecific(fut);
                                        return core::task::Poll::Pending;
                                    }
                                    core::task::Poll::Ready(version) => {
                                        let (reader, opts) = fut.take_reader();
                                        *self = Self::Done(reader, opts);
                                        return core::task::Poll::Ready(version.map(|version| {
                                            #name {
                                                version,
                                            }
                                        }));
                                    }
                                }
                            }
                            Self::Done(..) => panic!("poll after completion"),
                            Self::Empty => unreachable!(),
                        }
                    }
                }
            }

            impl #async_impl_generics TakeReader<'a, R> for #async_parse_enum_name #async_ty_generics #async_where_clause {
                impl_take_reader!{}
                fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                    match self {
                        Self::Version(v) => v.borrow_reader(),
                        Self::VersionSpecific(s) => s.borrow_reader(),
                        Self::Done(r, opts) => (r, opts),
                        Self::Empty => unreachable!(),
                    }
                }
            }

            impl #impl_generics AsyncParse for #name #ty_generics #where_clause {
                type Fut<'a, R: PollReader + Unpin> = #async_parse_enum_name #async_ty_generics;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    #async_parse_enum_name::Version(<#parse_repr>::create_fut(reader, options))
                }
            }
        }
    }
}
