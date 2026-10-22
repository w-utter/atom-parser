extern crate proc_macro;
use proc_macro::TokenStream;

mod definition;
use definition::{
    AtomDefinition, DefinitionKind, DefinitionList, EnumDefinition, NewTypeDefinition,
    StructDefinition,
};
mod field;
use field::{
    AtomField, AtomFields, StructFieldAttr, format_async_statemachine, format_parse_impl,
    format_parsed_ty,
};
mod flags;
use flags::{Flag, FlagList};
mod children;
use children::{Child, ChildList};
mod version;
use version::{Version, VersionList, Versioned, try_expand_into_versioned_fields};
mod fourcc;
use fourcc::FourCC;
mod conditional;
use conditional::{Conditionals, GroupedConditionals};

// used to format the base path of the module
// e.g ::atom_parse::trailing::TrailingIterator
// can be used by quote with `::#KRATE::trailing::` etc
struct CrateName(std::cell::LazyCell<syn::Ident>);

const KRATE: CrateName = CrateName(std::cell::LazyCell::new(|| {
    quote::format_ident!("atom_parser")
}));

impl quote::ToTokens for CrateName {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let this = &*self.0;
        this.to_tokens(tokens)
    }
}

#[proc_macro]
pub fn make_atom(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DefinitionList);

    use quote::quote;

    let defs = input.defs.into_iter().map(|def| {
        match def.kind {
            DefinitionKind::Struct(item) => {
                let mod_name = item.module_name();
                let StructDefinition {
                    attrs,
                    name,
                    fields,
                    generics,
                } = item;

                format_struct_impl(def.version, attrs, mod_name, name, fields, generics)
            }
            DefinitionKind::Atom(atom) => {
                let AtomDefinition {
                    fcc,
                    attrs,
                    name,
                    fields,
                } = atom;

                let atom_mod = fcc.as_ident();
                let [fcc_0, fcc_1, fcc_2, fcc_3] = fcc.as_fcc();

                let struct_impl = format_struct_impl(def.version, attrs, atom_mod, name.clone(), fields, Default::default());
                quote! {
                    #struct_impl

                    impl ::#KRATE::Atom for #name {
                        const FCC: ::#KRATE::fourcc::FourCC = ::#KRATE::fourcc::FourCC::new([#fcc_0, #fcc_1, #fcc_2, #fcc_3]);
                    }
                }
            }
            DefinitionKind::Enum(e) => {
                let EnumDefinition {
                    attrs,
                    name,
                    variants,
                    repr,
                } = e;

                let enum_variants = variants.iter().map(|v| &v.name);

                let const_bindings = variants.iter().enumerate().map(|(i, v)| {
                    let val = &v.val;
                    let variant = quote::format_ident!("VARIANT_{}", i);
                    quote! {
                        const #variant: #repr = #val;
                    }
                });

                let try_from_impl = variants.iter().enumerate().map(|(i, v)| {
                    let variant = quote::format_ident!("VARIANT_{}", i);
                    let name = &v.name;
                    quote!{
                        #variant => Self::#name
                    }
                });

                let async_parse_name = quote::format_ident!("Async{}Parse", name);

                quote! {
                    #(#attrs)*
                    pub enum #name {
                        #(#enum_variants,)*
                        Unknown(#repr),
                    }

                    impl #name {
                        pub fn try_from_bits(bits: #repr) -> Self {
                            #(#const_bindings)*
                            match bits {
                                #(#try_from_impl,)*
                                u => Self::Unknown(u),
                            }
                        }
                    }

                    impl ::#KRATE::parse::Parsed for #name {
                        type Output<__Offset> = #name;
                    }

                    impl <__Offset: ::#KRATE::reader::Offset> ::#KRATE::parse::Parse<__Offset> for #name {
                        fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>>(reader: &mut __Reader, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                            let repr = <#repr as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                            Ok(Self::try_from_bits(repr))
                        }
                    }

                    pub enum #async_parse_name<'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> {
                        Repr(<#repr as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<'__parse_options, __Reader>),
                        Done(__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions),
                        Empty,
                    }

                    impl <__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin> ::#KRATE::parse::AsyncParse<__Offset> for #name {
                        type Fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = #async_parse_name<'__parse_options, __Offset, __Reader>;
                        fn create_fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> <Self as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<'__parse_options, __Reader> {
                           #async_parse_name::Repr(<#repr as ::#KRATE::parse::AsyncParse<__Offset>>::create_fut(reader, opts))
                        }
                    }

                    impl <'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> Future for #async_parse_name<'__parse_options, __Offset, __Reader> {
                        type Output = ::core::result::Result<<#name as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError>;
                        fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                            let mut this = ::core::mem::replace(&mut *self, Self::Empty);
                            match this {
                                Self::Repr(mut fut) => {
                                    match ::core::pin::Pin::new(&mut fut).poll(cx) {
                                        ::core::task::Poll::Pending => {
                                            *self = Self::Repr(fut);
                                            return ::core::task::Poll::Pending;
                                        }
                                        ::core::task::Poll::Ready(res) => {
                                            let repr = res?;
                                            let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(fut);
                                            *self = Self::Done(reader, opts);
                                            return ::core::task::Poll::Ready(Ok(#name::try_from_bits(repr)));
                                        }
                                    }
                                }
                                Self::Done(..) => panic!("poll after completion"),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }

                    impl <'__parse_options, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<'__parse_options, __Reader> for #async_parse_name<'__parse_options, __Offset, __Reader> {
                        fn take_reader(self) -> (__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Done(reader, opts) => return (reader, opts),
                                _ => unreachable!("invalid state"),
                            }
                        }

                        fn borrow_reader(&mut self) -> (&mut __Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Repr(r) => r.borrow_reader(),
                                Self::Done(reader, opts) => (reader, opts),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }
                }
            }
            DefinitionKind::NewType(nt) => {
                // simplified type parsing
                // - all parsed output is itself
                //      - e.g, no stored offsets
                // - no support for field attrs
                let NewTypeDefinition {
                    attrs,
                    name,
                    generics,
                    tys,
                } = nt;

                let mut parse_generics = generics.clone();
                for param in &mut parse_generics.params {
                    if let syn::GenericParam::Type(ty) = param {
                        let ty_name = &ty.ident;
                        ty.bounds
                            .push(syn::parse_quote!(::#KRATE::parse::Parse<__Offset, Output<__Offset> = #ty_name>));
                    }
                }

                parse_generics
                    .params
                    .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset));

                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                let (parse_impl_generics, _, _) = parse_generics.split_for_impl();
                let parse_impl = tys.iter().enumerate().map(|(i, ty)| {
                    let field_name = quote::format_ident!("field_{}", i);
                    quote! {
                        let #field_name = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                    }
                });

                let collect_impl = tys.iter().enumerate().map(|(i, _)| {
                    quote::format_ident!("field_{}", i)
                }).collect::<Vec<_>>();

                let mut states = vec![];
                let mut poll_variant = vec![];
                let mut borrow_reader = vec![];

                for idx in 0..tys.len() {
                    let completed = &tys[0..idx];
                    let current = &tys[idx];

                    let field_names = completed.iter().enumerate().map(|(i, ty)| (quote::format_ident!("field_{i}"), ty)).collect::<Vec<_>>();
                    let field_as_state = field_names.iter().map(|(name, ty)| {
                        quote!(#name: #ty,)
                    });
                    let fut_name = quote::format_ident!("field_{idx}");
                    let current_as_state = quote!(#fut_name: <#current as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<'__parse_options, __Reader>,);
                    let state_name = quote::format_ident!("S{idx}");
                    states.push(quote!(#state_name { #(#field_as_state)* #current_as_state }));

                    let next_state = if let Some(next_ty) = tys.get(idx + 1) {
                        let new_collected = tys[0..=idx].iter().enumerate().map(|(i, _)| quote::format_ident!("field_{i}"));
                        let next_state = quote::format_ident!("S{}", idx + 1);
                        let next_field = quote::format_ident!("field_{}", idx + 1);
                        let next_fut = quote! {
                            <#next_ty as ::#KRATE::parse::AsyncParse<__Offset>>::create_fut(reader, opts)
                        };
                        quote! {
                            *self = Self::#next_state {
                                #(#new_collected,)*
                                #next_field: #next_fut,
                            };
                        }
                    } else {
                        let collected = tys.iter().enumerate().map(|(i, _)| quote::format_ident!("field_{i}"));

                        quote! {
                            *self = Self::Done(reader, opts);
                            return ::core::task::Poll::Ready(Ok(#name(#(#collected,)*)))
                        }
                    };

                    let completed_names = field_names.iter().map(|(name, _)| name).collect::<Vec<_>>();
                    let poll = quote! {
                        Self::#state_name { #(#completed_names,)* mut #fut_name } => {
                            match ::core::pin::Pin::new(&mut #fut_name).poll(cx) {
                                ::core::task::Poll::Pending => {
                                    *self = Self::#state_name { #(#completed_names,)* #fut_name };
                                    return ::core::task::Poll::Pending;
                                }
                                ::core::task::Poll::Ready(r) => {
                                    let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(#fut_name);
                                    let #fut_name = match r {
                                        Ok(#fut_name) => #fut_name,
                                        Err(e) => {
                                            *self = Self::Done(reader, opts);
                                            return ::core::task::Poll::Ready(Err(e));
                                        }
                                    };
                                    #next_state
                                }
                            }
                        }
                    };

                    let borrow_reader_impl = quote! {
                        Self::#state_name { #fut_name, ..} => #fut_name.borrow_reader(),
                    };

                    poll_variant.push(poll);
                    borrow_reader.push(borrow_reader_impl);
                }

                let pd_generics = generics.params.iter().filter_map(|p| match p {
                    syn::GenericParam::Type(t) => Some(&t.ident),
                    _ => None,
                });

                let (nop_state, nop_poll, nop_borrow) = if tys.is_empty() {
                    (
                        Some(quote! {
                            S0 { nop: (__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions, ::core::marker::PhantomData<( __Offset, #(#pd_generics,)* )>) },
                        }),
                        Some(quote! {
                            Self::S0 { nop: (reader, opts, _) } => {
                                return ::core::task::Poll::Ready(Ok(#name()));
                            }
                        }),
                        Some(quote! {
                            Self::S0 { nop: (reader, opts, _) } => (reader, opts),
                        }),
                    )
                } else {
                    (None, None, None)
                };

                let async_parse_name = quote::format_ident!("Async{name}Parse");
                let mut async_parse_generics = generics.clone();
                for param in &mut async_parse_generics.params {
                    if let syn::GenericParam::Type(ty) = param {
                        let ty_name = &ty.ident;
                        ty.bounds
                            .push(syn::parse_quote!(::#KRATE::parse::AsyncParse<__Offset, Output<__Offset> = #ty_name>));
                        ty.bounds
                            .push(syn::parse_quote!(::core::marker::Unpin));
                    }
                }

                async_parse_generics
                    .params
                    .push(syn::parse_quote!('__parse_options));
                async_parse_generics.params.push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin));
                async_parse_generics.params.push(syn::parse_quote!(__Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin));

                let async_statemachine = quote! {
                    pub enum #async_parse_name #async_parse_generics {
                        #(#states,)*
                        #nop_state
                        Done(__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions),
                        Empty,
                    }
                };

                let (async_parse_impl_generics, async_parse_ty_generics, async_parse_where_clause) = async_parse_generics.split_for_impl();

                let mut async_generics = generics.clone();
                for param in &mut async_generics.params {
                    if let syn::GenericParam::Type(ty) = param {
                        let ty_name = &ty.ident;
                        ty.bounds
                            .push(syn::parse_quote!(::#KRATE::parse::AsyncParse<__Offset, Output<__Offset> = #ty_name>));
                        ty.bounds
                            .push(syn::parse_quote!(::core::marker::Unpin));
                    }
                }

                let async_parse_start = if let Some(ty) = tys.first() {
                    quote! {
                        field_0: <#ty as ::#KRATE::parse::AsyncParse<__Offset>>::create_fut(reader, opts)
                    }
                } else {
                    quote! {
                        nop: (reader, opts, ::core::marker::PhantomData)
                    }
                };

                async_generics.params.push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin));

                let (async_impl_generics, _, _) = async_generics.split_for_impl();

                let field_tys = tys.iter().map(|ty| quote::quote!(pub #ty));
                quote! {
                    #(#attrs)*
                    pub struct #name #generics(#(#field_tys,)*);

                    impl #impl_generics ::#KRATE::parse::Parsed for #name #ty_generics #where_clause {
                        type Output<__Offset> = Self;
                    }

                    impl #parse_impl_generics ::#KRATE::parse::Parse<__Offset> for #name #ty_generics #where_clause {
                        fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>>(reader: &mut __Reader, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                            #(#parse_impl)*
                            Ok(Self(#(#collect_impl),*))
                        }
                    }

                    #async_statemachine

                    impl #async_parse_impl_generics Future for #async_parse_name #async_parse_ty_generics #async_parse_where_clause {
                        type Output = ::core::result::Result<#name #ty_generics, ::#KRATE::error::ParseError>;
                        fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                            loop {
                                let mut this = ::core::mem::replace(&mut *self, Self::Empty);
                                match this {
                                    #(#poll_variant)*
                                    #nop_poll
                                    Self::Done(..) => panic!("future polled after completion"),
                                    Self::Empty => unreachable!(),
                                }
                            }
                        }
                    }

                    impl #async_parse_impl_generics ::#KRATE::reader::TakeReader<'__parse_options, __Reader> for #async_parse_name #async_parse_ty_generics #async_parse_where_clause {
                        fn take_reader(self) -> (__Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Done(reader, opts) => return (reader, opts),
                                _ => unreachable!("invalid state"),
                            }
                        }

                        fn borrow_reader(&mut self) -> (&mut __Reader, &'__parse_options ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                #(#borrow_reader)*
                                #nop_borrow
                                Self::Done(reader, opts) => (reader, opts),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }

                    impl #async_impl_generics ::#KRATE::parse::AsyncParse<__Offset> for #name #ty_generics #where_clause {
                        type Fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = #async_parse_name #async_parse_ty_generics ;
                        fn create_fut<'__parse_options, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: &'__parse_options ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'__parse_options, __Reader> {
                            #async_parse_name ::S0 {
                                #async_parse_start
                            }
                        }
                    }
                }
            }
        }
    });

    TokenStream::from(quote! {
        #(#defs)*
    })
}

fn format_struct_impl(
    version: Option<syn::LitInt>,
    attrs: Vec<syn::Attribute>,
    mod_name: syn::Ident,
    name: syn::Ident,
    fields: Vec<AtomField>,
    mut generics: syn::Generics,
) -> proc_macro2::TokenStream {
    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds.push(syn::parse_quote!(::#KRATE::parse::Parsed));
        }
    }

    use quote::quote;

    let generic_collection = None;
    match try_expand_into_versioned_fields(fields, version) {
        Ok(Ok(versioned)) => {
            let version_specific = versioned.versions.iter().map(|(v, fields)| {
                let generics = generics.clone();
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                let payload_len = AtomFields::payload_len(fields);

                let version_mod = Versioned::version_mod_from_lit(v);


                let version_specific = fields.iter().filter_map(|field| field.as_inline_definition(Some(v), &attrs, &generics, generic_collection, payload_len.as_ref()));
                let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&mod_name, Some(&version_mod), payload_len.as_ref())).collect::<Vec<_>>();
                let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();
                let sync_helper_fns = fields.iter().filter_map(|f| f.as_sync_helper_fn(None)).collect::<Vec<_>>();
                let async_helper_fns = fields.iter().filter_map(|f| f.as_async_helper_fn(None)).collect::<Vec<_>>();

                let name = Versioned::versioned_struct_from_lit(&name, v);

                // so that if one version has a generic and another doesnt, all versions
                // are bounded by the same generics
                let (generic_storage, generic_collection) = if generics.params.is_empty() {
                    (None, None)
                } else {
                    let lts = generics.params.iter().filter_map(|param| {
                        if let syn::GenericParam::Lifetime(lt) = param {
                            Some(lt.lifetime.clone())
                        } else {
                            None
                        }
                    });
                    let rest = generics.params.iter().filter_map(|param| {
                        use syn::GenericParam;
                        Some(match param {
                            GenericParam::Type(ty) => ty.ident.clone(),
                            GenericParam::Const(c) => c.ident.clone(),
                            _ => return None,
                        })
                    });

                    let items = quote!(#(#lts,)*#(#rest,)*);
                    (
                        Some(quote!(_pd: ::core::marker::PhantomData< (( #items )) >)),
                        Some(quote!(_pd: ::core::marker::PhantomData,))
                    )
                };

                let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &generics, generic_collection.as_ref());
                let async_parse_impl = format_async_statemachine(fields, &name, generics.clone(), None, generic_collection.as_ref(), payload_len.as_ref());
                let fields = fields.iter().filter_map(|field| field.as_field_decl(None, payload_len.as_ref(), &generics)).collect::<Vec<_>>();

                let mut parse_generics = generics.clone();
                let mut async_parse_generics = generics.clone();
                let mut storage_generics = generics.clone();

                parse_generics.params.push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset = usize));
                async_parse_generics.params.push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize));
                storage_generics.params.push(syn::parse_quote!(__Offset = usize));
                let (parse_impl_generics, parse_ty_generics, parse_where_clause) = parse_generics.split_for_impl();
                let (async_parse_impl_generics, async_parse_ty_generics, async_parse_where_clause) = async_parse_generics.split_for_impl();

                quote! {
                    pub mod #version_mod {
                        use super::*;
                        #(#attrs)*
                        pub struct #name #storage_generics {
                            _offset: ::core::marker::PhantomData<__Offset>,
                            #(#fields,)*
                            #generic_storage
                        }

                        impl #parse_impl_generics #name #parse_ty_generics #parse_where_clause {
                            #(#sync_helper_fns)*
                        }

                        impl #async_parse_impl_generics #name #async_parse_ty_generics #async_parse_where_clause {
                            #(#async_helper_fns)*
                        }

                        impl #impl_generics ::#KRATE::parse::Parsed for #name #ty_generics #where_clause {
                            type Output<__Offset> = #name #parse_ty_generics ;
                        }

                        #parse_impl
                        #async_parse_impl
                        #(#version_specific)*
                    }
                }
            });

            let versions = versioned.format_versioned_struct(&name, &attrs, &generics);
            let enum_name = Versioned::format_enum_name_from_versioned_struct(&name);

            let async_parse_impl = versioned.format_async_parse(&name, &generics, &mod_name);

            let mut parse_generics = generics.clone();
            for param in &mut parse_generics.params {
                if let syn::GenericParam::Type(ty) = param {
                    ty.bounds
                        .push(syn::parse_quote!(::#KRATE::parse::Parse<__Offset>));
                }
            }

            let mut async_parse_generics = generics.clone();
            let mut storage_generics = generics.clone();

            parse_generics
                .params
                .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset = usize));
            async_parse_generics.params.push(
                syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize),
            );
            storage_generics
                .params
                .push(syn::parse_quote!(__Offset = usize));

            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            let (parse_impl_generics, parse_ty_generics, _) = parse_generics.split_for_impl();

            quote! {
                pub mod #mod_name {
                    #versions
                    #(#version_specific)*
                }

                #(#attrs)*
                pub struct #name #storage_generics {
                    pub version: #mod_name::#enum_name #parse_ty_generics,
                }

                impl #parse_impl_generics ::#KRATE::parse::Parse<__Offset> for #name #ty_generics #where_clause {
                    fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>>(reader: &mut __Reader, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                        let version = <#mod_name::#enum_name #ty_generics as ::#KRATE::parse::Parse<__Offset>>::parse(reader, opts)?;
                        Ok(#name {
                            version,
                        })
                    }
                }

                impl #impl_generics ::#KRATE::parse::Parsed for #name #ty_generics #where_clause {
                    type Output<__Offset> = #name #parse_ty_generics ;
                }

                #async_parse_impl
            }
        }
        Ok(Err(fields)) => {
            let payload_len = AtomFields::payload_len(&fields);
            // since some fields are omitted
            // (e.g reserved/padding)
            let atom_fields = fields
                .iter()
                .filter_map(|field| {
                    field.as_field_decl(Some(&mod_name), payload_len.as_ref(), &generics)
                })
                .collect::<Vec<_>>();

            let mod_specific = AtomFields::group_inline_definitions(
                &fields,
                &mod_name,
                None,
                &attrs,
                &generics,
                generic_collection,
                payload_len.as_ref(),
            );
            let attrs = attrs.iter();

            let sync_parsing = fields
                .iter()
                .filter_map(|f| f.as_sync_parse(&mod_name, None, payload_len.as_ref()))
                .collect::<Vec<_>>();
            let field_collection = fields
                .iter()
                .filter_map(|f| f.as_collection())
                .collect::<Vec<_>>();

            let sync_helper_fns = fields
                .iter()
                .filter_map(|f| f.as_sync_helper_fn(Some(&mod_name)))
                .collect::<Vec<_>>();
            let async_helper_fns = fields
                .iter()
                .filter_map(|f| f.as_async_helper_fn(Some(&mod_name)))
                .collect::<Vec<_>>();

            let parse_impl =
                format_parse_impl(&name, &sync_parsing, &field_collection, &generics, None);
            let async_parse_impl = format_async_statemachine(
                &fields,
                &name,
                generics.clone(),
                Some(&mod_name),
                None,
                payload_len.as_ref(),
            );

            let parsed_output = format_parsed_ty(&name, &generics);

            let mut storage_generics = generics.clone();

            let mut async_generics = generics.clone();
            generics
                .params
                .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset = usize));
            async_generics.params.push(
                syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize),
            );
            storage_generics
                .params
                .push(syn::parse_quote!(__Offset = usize));

            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            let (async_impl_generics, async_ty_generics, async_where_clause) =
                async_generics.split_for_impl();

            quote! {
                #(#attrs)*
                pub struct #name #storage_generics {
                    _offset: ::core::marker::PhantomData<__Offset>,
                    #(#atom_fields),*
                }

                impl #impl_generics #name #ty_generics #where_clause {
                    #(#sync_helper_fns)*
                }

                impl #async_impl_generics #name #async_ty_generics #async_where_clause {
                    #(#async_helper_fns)*
                }

                #parsed_output

                #parse_impl

                #async_parse_impl

                #mod_specific
            }
        }
        Err(e) => {
            panic!("{e}")
        }
    }
}
