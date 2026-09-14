extern crate proc_macro;
use proc_macro::TokenStream;

mod definition;
use definition::{
    AtomDefinition, DefinitionKind, DefinitionList, EnumDefinition, StructDefinition,
};
mod field;
use field::{AtomField, AtomFields, StructFieldAttr, format_async_statemachine, format_parse_impl};
mod flags;
use flags::{Flag, FlagList};
mod children;
use children::{Child, ChildList};
mod version;
use version::{Version, VersionList, Versioned, try_expand_into_versioned_fields};
mod fourcc;
use fourcc::FourCC;

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
            DefinitionKind::Struct(mut item) => {
                for param in &mut item.generics.params {
                    if let syn::GenericParam::Type(type_param) = param {
                        type_param.bounds.push(syn::parse_quote!(::#KRATE::parse::Parse));
                        type_param.bounds.push(syn::parse_quote!(::core::marker::Unpin));
                    }
                }

                let atom_mod = item.module_name();
                let StructDefinition {
                    attrs,
                    name,
                    fields,
                    generics,
                } = item;

                match try_expand_into_versioned_fields(fields, def.version) {
                    Ok(Ok(versioned)) => {
                        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                        let version_specific = versioned.versions.iter().map(|(v, fields)| {
                            let version_mod = Versioned::version_mod_from_lit(v);

                            let version_specific = fields.iter().filter_map(|field| field.as_inline_definition(Some(v)));
                            let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, Some(&version_mod))).collect::<Vec<_>>();
                            let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();
                            let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(None)).collect::<Vec<_>>();

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
                            let async_parse_impl = format_async_statemachine(fields, &name, generics.clone(), None, generic_collection.as_ref());
                            let fields = fields.iter().filter_map(|field| field.as_field_decl(None));

                            quote! {
                                pub mod #version_mod {
                                    use super::*;
                                    #(#attrs)*
                                    #[derive(Debug)]
                                    pub struct #name #generics {
                                        #(#fields,)*
                                        #generic_storage
                                    }

                                    impl #impl_generics #name #ty_generics #where_clause{
                                        #(#helper_fns)*
                                    }

                                    #parse_impl
                                    #async_parse_impl
                                    #(#version_specific)*
                                }
                            }
                        });

                        let mut generics = generics.clone();

                        for param in &mut generics.params {
                            if let syn::GenericParam::Type(ty) = param {
                                ty.bounds.push(syn::parse_quote!(Unpin));
                            }
                        }

                        let versions = versioned.format_versioned_struct(&name, &attrs, &generics);
                        let enum_name = Versioned::format_enum_name_from_versioned_struct(&name);

                        let async_parse_impl = versioned.format_async_parse(&name, &generics, &atom_mod);

                        quote! {
                            pub mod #atom_mod {
                                #versions
                                #(#version_specific)*
                            }

                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name #generics {
                                pub version: #atom_mod::#enum_name #ty_generics,
                            }

                            impl #impl_generics ::#KRATE::parse::Parse for #name #ty_generics #where_clause {
                                fn parse<T: ::#KRATE::reader::Reader>(reader: &mut T, options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                                    let version = <#atom_mod::#enum_name as ::#KRATE::parse::Parse>::parse(reader, options)?;
                                    Ok(Self {
                                        version,
                                    })
                                }
                            }
                            #async_parse_impl
                        }
                    }
                    Ok(Err(fields)) => {
                        let attrs = attrs.iter();
                        // since some fields are omitted
                        // (e.g reserved/padding)
                        let atom_fields = fields.iter().filter_map(|field| field.as_field_decl(Some(&atom_mod)));

                        let mod_specific = AtomFields::group_inline_definitions(&fields, &atom_mod, None);

                        let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, None)).collect::<Vec<_>>();
                        let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();

                        let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(Some(&atom_mod))).collect::<Vec<_>>();

                        let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &generics, None);
                        let async_parse_impl = format_async_statemachine(&fields, &name, generics.clone(), Some(&atom_mod), None);

                        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                        quote! {
                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name #generics {
                                #(#atom_fields),*
                            }

                            impl #impl_generics #name #ty_generics #where_clause {
                                #(#helper_fns)*
                            }

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
            DefinitionKind::Atom(atom) => {
                let AtomDefinition {
                    fcc,
                    attrs,
                    name,
                    fields,
                } = atom;

                let atom_mod = fcc.as_ident();
                let [fcc_0, fcc_1, fcc_2, fcc_3] = fcc.as_fcc();
                match try_expand_into_versioned_fields(fields, def.version) {
                    Ok(Ok(versioned)) => {
                        let version_specific = versioned.versions.iter().map(|(v, fields)| {
                            let version_mod = Versioned::version_mod_from_lit(v);

                            let version_specific = fields.iter().filter_map(|field| field.as_inline_definition(Some(v)));
                            let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, Some(&version_mod))).collect::<Vec<_>>();
                            let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();
                            let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(None)).collect::<Vec<_>>();

                            let name = Versioned::versioned_struct_from_lit(&name, v);

                            let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &Default::default(), None);
                            let async_parse_impl = format_async_statemachine(fields, &name, Default::default(), None, None);
                            let fields = fields.iter().filter_map(|field| field.as_field_decl(None));

                            quote! {
                                pub mod #version_mod {
                                    use super::*;
                                    #(#attrs)*
                                    #[derive(Debug)]
                                    pub struct #name {
                                        #(#fields),*
                                    }

                                    impl #name {
                                        #(#helper_fns)*
                                    }

                                    #parse_impl
                                    #async_parse_impl
                                    #(#version_specific)*
                                }
                            }
                        });

                        let versions = versioned.format_versioned_struct(&name, &attrs, &Default::default());
                        let enum_name = Versioned::format_enum_name_from_versioned_struct(&name);

                        let async_parse_impl = versioned.format_async_parse(&name, &Default::default(), &atom_mod);

                        quote! {
                            pub mod #atom_mod {
                                #versions
                                #(#version_specific)*
                            }

                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name {
                                pub version: #atom_mod::#enum_name,
                            }

                            impl ::#KRATE::parse::Parse for #name {
                                fn parse<T: ::#KRATE::reader::Reader>(reader: &mut T, options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                                    let version = <#atom_mod::#enum_name as ::#KRATE::parse::Parse>::parse(reader, options)?;
                                    Ok(Self {
                                        version,
                                    })
                                }
                            }

                            #async_parse_impl

                            impl ::#KRATE::Atom for #name {
                                const FCC: ::#KRATE::fourcc::FourCC = ::#KRATE::fourcc::FourCC::new([#fcc_0, #fcc_1, #fcc_2, #fcc_3]);
                            }
                        }
                    }
                    Ok(Err(fields)) => {
                        let attrs = attrs.iter();
                        // since some fields are omitted
                        // (e.g reserved/padding)
                        let atom_fields = fields.iter().filter_map(|field| field.as_field_decl(Some(&atom_mod)));

                        let mod_specific = AtomFields::group_inline_definitions(&fields, &atom_mod, None);

                        let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, None)).collect::<Vec<_>>();
                        let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();

                        let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(Some(&atom_mod))).collect::<Vec<_>>();

                        let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &Default::default(), None);
                        let async_parse_impl = format_async_statemachine(&fields, &name, Default::default(), Some(&atom_mod), None);

                        quote! {
                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name {
                                #(#atom_fields),*
                            }

                            impl #name {
                                #(#helper_fns)*
                            }

                            impl ::#KRATE::Atom for #name {
                                const FCC: ::#KRATE::fourcc::FourCC = ::#KRATE::fourcc::FourCC::new([#fcc_0, #fcc_1, #fcc_2, #fcc_3]);
                            }

                            #parse_impl
                            #async_parse_impl

                            #mod_specific
                        }
                    }
                    Err(e) => panic!("{e}"),
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

                let try_from_impl = variants.iter().map(|v| {
                    let val = &v.val;
                    let name = &v.name;
                    quote!{
                        #val => Self::#name
                    }
                });

                let async_parse_name = quote::format_ident!("Async{}Parse", name);

                quote! {
                    #(#attrs)*
                    #[derive(Debug)]
                    pub enum #name {
                        #(#enum_variants,)*
                        Unknown(#repr),
                    }

                    impl #name {
                        pub fn try_from_bits(bits: #repr) -> Self {
                            match bits {
                                #(#try_from_impl,)*
                                u => Self::Unknown(u),
                            }
                        }
                    }

                    impl ::#KRATE::parse::Parse for #name {
                        fn parse<T: ::#KRATE::reader::Reader>(reader: &mut T, options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                            let repr = <#repr as ::#KRATE::parse::Parse>::parse(reader, options)?;
                            Ok(Self::try_from_bits(repr))
                        }
                    }

                    pub enum #async_parse_name<'a, R: PollReader + Unpin> {
                        Repr(<#repr as ::#KRATE::parse::AsyncParse>::Fut<'a, R>),
                        Done(R, &'a ParseOptions),
                        Empty,
                    }

                    impl AsyncParse for #name {
                        type Fut<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> = #async_parse_name<'a, R>;
                        fn create_fut<'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin>(reader: R, options: &'a ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'a, R> {
                           #async_parse_name::Repr(<#repr as ::#KRATE::parse::AsyncParse>::create_fut(reader, options))
                        }
                    }

                    impl <'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> Future for #async_parse_name<'a, R> {
                        type Output = ::core::result::Result<#name, ::#KRATE::error::ParseError>;
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

                    impl <'a, R: ::#KRATE::reader::PollReader + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<'a, R> for #async_parse_name<'a, R> {
                        fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Done(reader, opts) => return (reader, opts),
                                _ => unreachable!("invalid state"),
                            }
                        }

                        fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                Self::Repr(r) => r.borrow_reader(),
                                Self::Done(reader, opts) => (reader, opts),
                                Self::Empty => unreachable!(),
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
