use crate::{KRATE, VersionList};
use std::collections::HashSet;

#[derive(Clone)]
pub enum TagOverride {
    Tag(syn::Expr),
    FourCC(crate::FourCC),
}

#[derive(Clone)]
pub struct Child {
    pub specified_type: Option<syn::Type>,
    pub name: syn::Ident,
    pub version_num: HashSet<syn::LitInt>,
    pub tag_override: Option<TagOverride>,
}

impl syn::parse::Parse for Child {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;

        let mut version_num = None;
        let mut tag_override = None;
        for attr in attrs.into_iter() {
            let path = attr.path();
            if path.is_ident("version") {
                if version_num.is_some() {
                    panic!("duplicate version attrs")
                }
                version_num = Some(VersionList::parse_version_num_from_attr(attr)?);
            } else if path.is_ident("override_tag") {
                if tag_override.is_some() {
                    panic!("duplicate tag override");
                }
                tag_override = Some(TagOverride::Tag(attr.parse_args()?));
            } else if path.is_ident("override_fourcc") {
                if tag_override.is_some() {
                    panic!("duplicate tag override");
                }
                tag_override = Some(TagOverride::FourCC(crate::FourCC::from_expr(
                    attr.parse_args()?,
                )?));
            } else {
                panic!("unknown attr");
            }
        }
        let version_num = version_num.unwrap_or_default();

        let f = || -> syn::Result<_> {
            let fork = input.fork();

            if fork.parse::<syn::Type>().is_ok() && fork.peek(syn::Token![as]) {
                let ty = input.parse()?;
                let _: syn::Token![as] = input.parse()?;
                let name = input.parse()?;
                return Ok((name, Some(ty)));
            }
            Ok((input.parse()?, None))
        };
        let (name, specified_type) = f()?;
        Ok(Self {
            name,
            specified_type,
            version_num,
            tag_override,
        })
    }
}

pub struct ChildList {
    pub inner: Vec<Child>,
}

impl syn::parse::Parse for ChildList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let content = content.parse_terminated(Child::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();
        Ok(Self { inner })
    }
}

impl ChildList {
    fn filter_by_version<'a>(
        children: &'a [Child],
        version: Option<&'a syn::LitInt>,
    ) -> impl Iterator<Item = &'a Child> {
        children.iter().filter_map(move |child| {
            if let Some(v) = version {
                if !child.version_num.is_empty() && !child.version_num.contains(v) {
                    return None;
                }
            } else if !child.version_num.is_empty() {
                panic!("child version but no version identifier")
            }
            Some(child)
        })
    }

    fn ty_from_child(child: &Child) -> proc_macro2::TokenStream {
        use quote::quote;
        let name = &child.name;
        if let Some(ty) = child.specified_type.as_ref() {
            quote!(#ty)
        } else {
            quote!(#name)
        }
    }

    pub fn bind_overridden_tag_consts(
        children: &[Child],
        version: Option<&syn::LitInt>,
        tag_type: &syn::Type,
    ) -> Vec<proc_macro2::TokenStream> {
        use quote::quote;
        Self::filter_by_version(children, version)
            .enumerate()
            .filter_map(|(i, child)| {
                let val = match child.tag_override.as_ref()? {
                    TagOverride::Tag(expr) => quote!(#expr),
                    TagOverride::FourCC(fcc) => {
                        let [fcc_0, fcc_1, fcc_2, fcc_3] = fcc.as_fcc();
                        quote!(::#KRATE::fourcc::FourCC::new([#fcc_0, #fcc_1, #fcc_2, #fcc_3]))
                    }
                };
                let name = Self::format_overridden_tag(i);
                Some(quote!(const #name: #tag_type = #val;))
            })
            .collect()
    }

    fn format_overridden_tag(idx: usize) -> syn::Ident {
        quote::format_ident!("OVERRIDDEN_TAG_{idx}")
    }

    fn ty_and_tag_from_child(
        child: &Child,
        tag_type: &syn::Type,
        idx: usize,
        atom: bool,
    ) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
        use quote::quote;
        let ty = Self::ty_from_child(child);

        let tag = if child.tag_override.is_some() {
            let tag = Self::format_overridden_tag(idx);
            quote!(#tag)
        } else if atom {
            quote! {
                <#ty as ::#KRATE::tag::Atom>::FCC
            }
        } else {
            quote! {
                <#ty as ::#KRATE::tag::Tag<#tag_type>>::TAG
            }
        };
        (ty, tag)
    }

    pub fn sync_parse(
        children: &[Child],
        version: Option<&syn::LitInt>,
        tag_type: &syn::Type,
        mod_name: Option<&proc_macro2::TokenStream>,
        tagged_name: &syn::Ident,
        atom: bool,
        skip_seek: bool,
    ) -> Vec<proc_macro2::TokenStream> {
        assert!(!(atom && skip_seek));
        use quote::quote;
        let mod_name = mod_name.map(|m| quote!(#m ::));

        let seek =
            (!skip_seek).then(|| quote!(::#KRATE::reader::Reader::seek_remaining(&mut reader)?;));

        Self::filter_by_version(children, version)
        .enumerate()
        .map(|(i, child)| {
            let name = &child.name;
            let (ty, tag) = Self::ty_and_tag_from_child(child, tag_type, i, atom);

            quote!{
                #tag => {
                    let tagged = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(&mut reader, opts)?;
                    #seek
                    #mod_name #tagged_name::#name(tagged)
                }
            }
        }).collect()
    }

    pub fn async_parse(
        children: &[Child],
        version: Option<&syn::LitInt>,
        tag_type: &syn::Type,
        mod_name: Option<&proc_macro2::TokenStream>,
        tagged_name: &syn::Ident,
        atom: bool,
    ) -> Vec<proc_macro2::TokenStream> {
        use quote::quote;
        Self::filter_by_version(children, version)
        .enumerate()
        .map(|(i, child)| {
            let name = &child.name;
            let (ty, tag) = Self::ty_and_tag_from_child(child, tag_type, i, atom);

            quote!{
                #tag => {
                    #mod_name #tagged_name :: #name (<#ty as ::#KRATE::parse::AsyncParse<__Offset>>::create_fut(reader, opts))
                }
            }
        }).collect()
    }

    pub fn async_statemachine(
        children: &[Child],
        version: Option<&syn::LitInt>,
        trailing_reader: bool,
    ) -> Vec<proc_macro2::TokenStream> {
        use quote::quote;

        let reader = if trailing_reader {
            quote!(::#KRATE::reader::TrailingReader<__Offset, __Reader>)
        } else {
            quote!(__Reader)
        };

        Self::filter_by_version(children, version)
            .map(|child| {
                let name = &child.name;
                let ty = Self::ty_from_child(child);

                quote! {
                    #name(<#ty as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<__Options, #reader>),
                }
            })
            .collect()
    }

    pub fn tagged_variants(
        children: &[Child],
        version: Option<&syn::LitInt>,
    ) -> Vec<proc_macro2::TokenStream> {
        use quote::quote;

        Self::filter_by_version(children, version)
            .map(|child| {
                let name = &child.name;
                let ty = Self::ty_from_child(child);

                quote! {
                    #name(<#ty as ::#KRATE::parse::Parsed>::Output<__Offset>),
                }
            })
            .collect()
    }

    pub fn borrow_reader_variants(
        children: &[Child],
        version: Option<&syn::LitInt>,
        trailing_reader: bool,
    ) -> Vec<proc_macro2::TokenStream> {
        use quote::quote;
        let borrow_reader = if trailing_reader {
            quote! {
                {
                    let (reader, opts) = fut.borrow_reader();
                    (&mut reader.reader, opts)
                }
            }
        } else {
            quote! {
                fut.borrow_reader(),
            }
        };

        Self::filter_by_version(children, version)
            .map(|child| {
                let name = &child.name;

                quote! {
                    Self::#name(fut) => #borrow_reader
                }
            })
            .collect()
    }

    pub fn async_statemachine_poll(
        children: &[Child],
        version: Option<&syn::LitInt>,
        children_name: &syn::Ident,
        skip_seek: bool,
    ) -> Vec<proc_macro2::TokenStream> {
        use quote::quote;

        let state_transition = if skip_seek {
            quote! {
                *self = Self::Done(reader, opts);
                return ::core::task::Poll::Ready(Ok(child))
            }
        } else {
            quote! {
                let rest = ::#KRATE::reader::PollReader::remaining_size(&reader);
                *self = Self::Seek(child, ::#KRATE::reader::AsyncSeek::seek(reader, rest), opts);
            }
        };

        Self::filter_by_version(children, version)
            .map(|child| {
                let name = &child.name;

                quote! {
                    Self::#name(mut fut) => {
                        match ::core::pin::Pin::new(&mut fut).poll(cx) {
                            ::core::task::Poll::Pending => {
                                *self = Self::#name (fut);
                                return ::core::task::Poll::Pending;
                            }
                            ::core::task::Poll::Ready(res) => {
                                let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(fut);
                                let child = match res {
                                    Ok(child) => child,
                                    Err(e) => {
                                        *self = Self::Done(reader, opts);
                                        return ::core::task::Poll::Ready(Err(e));
                                    }
                                };
                                let child = #children_name::#name(child);
                                #state_transition
                            }
                        }
                    }
                }
            })
            .collect()
    }
}
