extern crate proc_macro;
use proc_macro::TokenStream;

mod definition;
use definition::{
    AtomDefinition, DefinitionKind, DefinitionList, EnumDefinition, StructDefinition,
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

                    impl ::#KRATE::parse::Parsed for #name {
                        type Output<O> = #name;
                    }

                    impl <O: ::#KRATE::reader::Offset> ::#KRATE::parse::Parse<O> for #name {
                        fn parse<T: ::#KRATE::reader::Reader<O>>(reader: &mut T, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<O>, ::#KRATE::error::ParseError> {
                            let repr = <#repr as ::#KRATE::parse::Parse<O>>::parse(reader, opts)?;
                            Ok(Self::try_from_bits(repr))
                        }
                    }

                    pub enum #async_parse_name<'a, O: ::#KRATE::reader::Offset + ::core::marker::Unpin, R: ::#KRATE::reader::PollReader<O> + ::core::marker::Unpin> {
                        Repr(<#repr as ::#KRATE::parse::AsyncParse<O>>::Fut<'a, R>),
                        Done(R, &'a ::#KRATE::parse_options::ParseOptions),
                        Empty,
                    }

                    impl <O: ::#KRATE::reader::Offset + ::core::marker::Unpin> ::#KRATE::parse::AsyncParse<O> for #name {
                        type Fut<'a, R: ::#KRATE::reader::PollReader<O> + ::core::marker::Unpin> = #async_parse_name<'a, O, R>;
                        fn create_fut<'a, R: ::#KRATE::reader::PollReader<O> + ::core::marker::Unpin>(reader: R, opts: &'a ::#KRATE::parse_options::ParseOptions) -> <Self as ::#KRATE::parse::AsyncParse<O>>::Fut<'a, R> {
                           #async_parse_name::Repr(<#repr as ::#KRATE::parse::AsyncParse<O>>::create_fut(reader, opts))
                        }
                    }

                    impl <'a, O: ::#KRATE::reader::Offset + ::core::marker::Unpin, R: ::#KRATE::reader::PollReader<O> + ::core::marker::Unpin> Future for #async_parse_name<'a, O, R> {
                        type Output = ::core::result::Result<<#name as ::#KRATE::parse::Parsed>::Output<O>, ::#KRATE::error::ParseError>;
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

                    impl <'a, O: ::#KRATE::reader::Offset + ::core::marker::Unpin, R: ::#KRATE::reader::PollReader<O> + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<'a, R> for #async_parse_name<'a, O, R> {
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

                parse_generics.params.push(syn::parse_quote!(O: ::#KRATE::reader::Offset = usize));
                async_parse_generics.params.push(syn::parse_quote!(O: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize));
                storage_generics.params.push(syn::parse_quote!(O = usize));
                let (parse_impl_generics, parse_ty_generics, parse_where_clause) = parse_generics.split_for_impl();
                let (async_parse_impl_generics, async_parse_ty_generics, async_parse_where_clause) = async_parse_generics.split_for_impl();

                quote! {
                    pub mod #version_mod {
                        use super::*;
                        #(#attrs)*
                        pub struct #name #storage_generics {
                            _offset: ::core::marker::PhantomData<O>,
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
                            type Output<O> = #name #parse_ty_generics ;
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
                    ty.bounds.push(syn::parse_quote!(::#KRATE::parse::Parse<O>));
                }
            }

            let mut async_parse_generics = generics.clone();
            let mut storage_generics = generics.clone();

            parse_generics
                .params
                .push(syn::parse_quote!(O: ::#KRATE::reader::Offset = usize));
            async_parse_generics.params.push(
                syn::parse_quote!(O: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize),
            );
            storage_generics.params.push(syn::parse_quote!(O = usize));

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

                impl #parse_impl_generics ::#KRATE::parse::Parse<O> for #name #ty_generics #where_clause {
                    fn parse<T: ::#KRATE::reader::Reader<O>>(reader: &mut T, opts: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<O>, ::#KRATE::error::ParseError> {
                        let version = <#mod_name::#enum_name #ty_generics as ::#KRATE::parse::Parse<O>>::parse(reader, opts)?;
                        Ok(#name {
                            version,
                        })
                    }
                }

                impl #impl_generics ::#KRATE::parse::Parsed for #name #ty_generics #where_clause {
                    type Output<O> = #name #parse_ty_generics ;
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
                .push(syn::parse_quote!(O: ::#KRATE::reader::Offset = usize));
            async_generics.params.push(
                syn::parse_quote!(O: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize),
            );
            storage_generics.params.push(syn::parse_quote!(O = usize));

            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            let (async_impl_generics, async_ty_generics, async_where_clause) =
                async_generics.split_for_impl();

            quote! {
                #(#attrs)*
                pub struct #name #storage_generics {
                    _offset: ::core::marker::PhantomData<O>,
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
