use std::collections::HashSet;
use crate::{VersionList, KRATE};

#[derive(Clone)]
pub struct Flag {
    pub name: syn::Ident,
    pub val: syn::Expr,
    pub expected: bool,
    pub version_num: HashSet<syn::LitInt>,
}

impl syn::parse::Parse for Flag {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        if attrs.len() > 2 {
            panic!("more than 2 attrs specified for flag")
        }

        let mut expected = None;
        let mut version_num = None;
        for attr in attrs.into_iter() {
            let path = attr.path();
            if path.is_ident("expected") {
                if expected.is_some() {
                    panic!("duplicate expected attrs")
                }
                expected = Some(true)
            } else if path.is_ident("version") {
                if version_num.is_some() {
                    panic!("duplicate version attrs")
                }
                version_num = Some(VersionList::parse_version_num_from_attr(attr)?)
            } else {
                panic!("unknown attr for field")
            }
        }
        let version_num = version_num.unwrap_or_default();
        let expected = expected.unwrap_or_default();

        let _: syn::Token![const] = input.parse()?;
        let name = input.parse()?;
        let _: syn::Token![=] = input.parse()?;
        let val = input.parse()?;
        Ok(Self {
            name,
            val,
            expected,
            version_num,
        })
    }
}

pub struct FlagList {
    pub flags: Vec<Flag>,
}

impl FlagList {
    pub fn can_elide_flag_storage(flags: &[Flag], version: Option<&syn::LitInt>) -> bool {
        if flags.iter().filter(|flag| {
            if let Some(v) = version {
                if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                    return false;
                }
            } else if !flag.version_num.is_empty() {
                panic!("version specified for flags but no version identifier")
            }
            true
        }).any(|flag| !flag.expected) {
            return false
        }
        true
    }

    pub fn flags_impl<'a>(name: &'a syn::Ident, flags: &'a [Flag], repr: &'a syn::Type, version: Option<&'a syn::LitInt>) -> impl Iterator<Item = proc_macro2::TokenStream> + 'a {
        flags.iter().filter_map(move |flag| {
            if let Some(v) = version {
                if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                    return None;
                }
            } else if !flag.version_num.is_empty() {
                panic!("version specified for flags but no version identifier")
            }

            let flag_name = &flag.name;
            let flag_val = &flag.val;
            Some(quote::quote!(pub const #flag_name: #name = Self::from_bits_(#flag_val as #repr);))
        })
    }

    pub fn flags_trait_impl(name: &syn::Ident, flags: &[Flag], parse_repr: &syn::Type, version: Option<&syn::LitInt>, can_elide_flags: bool) -> proc_macro2::TokenStream {
        use quote::quote;
        let flags_check = Self::flags_parse_check(flags, version);

        let storage_attr = if can_elide_flags {
            Some(quote!(#[cfg(feature = "store_unknown_fields")]))
        } else {
            None
        };

        quote! {
            impl ::#KRATE::flags::FlagsParse<#parse_repr> for #name {
                fn try_from_bits(bits: #parse_repr, options: &::#KRATE::parse_options::ParseOptions) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                    #flags_check

                    Ok(Self {
                        #storage_attr
                        inner: bits,
                    })
                }
            }
        }
    }

    pub fn flags_bitops_impl(name: &syn::Ident) -> proc_macro2::TokenStream {
        quote::quote! {
            impl ::core::ops::BitAnd<#name> for #name {
                type Output = #name;
                fn bitand(self, rhs: #name) -> Self::Output {
                    ::#KRATE::flags::Flags::from_bits(::#KRATE::flags::Flags::to_bits(self) & ::#KRATE::flags::Flags::to_bits(rhs))
                }
            }

            impl ::core::ops::BitOr<#name> for #name {
                type Output = #name;
                fn bitor(self, rhs: #name) -> Self::Output {
                    ::#KRATE::flags::Flags::from_bits(::#KRATE::flags::Flags::to_bits(self) | ::#KRATE::flags::Flags::to_bits(rhs))
                }
            }

            impl ::core::ops::BitXor<#name> for #name {
                type Output = #name;
                fn bitxor(self, rhs: #name) -> Self::Output {
                    ::#KRATE::flags::Flags::from_bits(::#KRATE::flags::Flags::to_bits(self) ^ ::#KRATE::flags::Flags::to_bits(rhs))
                }
            }

            impl ::core::ops::Not for #name {
                type Output = #name;
                fn not(self) -> Self::Output {
                    ::#KRATE::flags::Flags::from_bits(!::#KRATE::flags::Flags::to_bits(self))
                }
            }
        }
    }

    pub fn flags_parse_check(flags: &[Flag], version: Option<&syn::LitInt>) -> proc_macro2::TokenStream {
        use quote::quote;
        let current_flags = flags.iter().filter(|flag| {
            if let Some(v) = version {
                if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                    return false;
                }
            } else if !flag.version_num.is_empty() {
                panic!("version specified for flags but no version identifier")
            }

            true
        }).collect::<Vec<_>>();

        let expected_check = if current_flags.iter().filter(|flag| flag.expected).count() > 0 {
            let expected_flags = current_flags.iter().filter_map(|flag| {
                if flag.expected {
                    Some(&flag.val)
                } else {
                    None
                }
            }).collect::<Vec<_>>();

            Some(quote! {
                if options.error_on_missing_flags && ((bits & (#((#expected_flags))|*)) != (#((#expected_flags))|*)) {
                    return Err(::#KRATE::error::ParseError::MissingFlags);
                }
            })
        } else {
            None
        };

        let check = if !current_flags.is_empty() {
            let flags = current_flags.iter().map(|flag| &flag.val);
            Some(quote! {
                if (options.error_on_unknown_flags && ((bits & (#((#flags))|*))) != 0) {
                    return Err(::#KRATE::error::ParseError::UnknownFlags);
                }
            })
        } else {
            None
        };

        quote! {
            #expected_check
            #check
        }
    }

    pub fn debug_impl(flags: &[Flag], name: &syn::Ident, version: Option<&syn::LitInt>, can_be_elided: bool) -> proc_macro2::TokenStream {
        use quote::quote;

        let flags_debug_elided = flags.iter().filter_map(|flag| {
            if let Some(v) = version {
                if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                    return None;
                }
            } else if !flag.version_num.is_empty() {
                panic!("version specified for flags but no version identifier")
            }

            let name = &flag.name;
            Some(quote! {
                if !first {
                    write!(f, " | ")?;
                }
                write!(f, "{}", stringify!(#name))?;
                first = false;
            })
        });

        let flags_debug = flags.iter().filter_map(|flag| {
            if let Some(v) = version {
                if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                    return None;
                }
            } else if !flag.version_num.is_empty() {
                panic!("version specified for flags but no version identifier")
            }

            let flag_val = &flag.val;
            let name = &flag.name;
            Some(quote! {
                if val & (#flag_val) == (#flag_val) {
                    if !first {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", stringify!(#name))?;
                    first = false;
                }
            })
        });

        let debug_impl = if can_be_elided {
            quote! {
                #[cfg(feature = "store_unknown_fields")]
                {
                    #(#flags_debug_elided)*
                }
                #[cfg(not(feature = "store_unknown_fields"))]
                {
                    #(#flags_debug)*
                }
            }
        } else {
            quote! {
                #(#flags_debug)*
            }
        };

        quote! {
            impl ::core::fmt::Debug for #name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let val = ::#KRATE::flags::Flags::to_bits(*self);
                    let mut first = true;
                    write!(f, "{}(", stringify!(#name))?;
                    #debug_impl
                    write!(f, ")")?;
                    Ok(())
                }
            }
        }
    }

    pub fn async_parse_impl(name: &syn::Ident, parse_repr: &syn::Type) -> proc_macro2::TokenStream {
        let parse_name = quote::format_ident!("Async{}Parse", name);
        quote::quote! {
            pub struct #parse_name<'a, R: ::#KRATE::reader::PollReader + Unpin> {
                inner: <#parse_repr as ::#KRATE::parse::AsyncParse>::Fut<'a, R>,
            }

            impl <'a, R: ::#KRATE::reader::PollReader + Unpin> Future for #parse_name <'a, R> {
                type Output = ::core::result::Result<#name, ::#KRATE::error::ParseError>;
                fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                    let bits = ::core::task::ready!(::core::pin::Pin::new(&mut self.inner).poll(cx))?;
                    let (_, opts) = ::#KRATE::reader::TakeReader::borrow_reader(&mut*self);
                    let flags = <#name as ::#KRATE::flags::FlagsParse<#parse_repr>>::try_from_bits(bits, opts)?;
                    ::core::task::Poll::Ready(Ok(flags))
                }
            }

            impl ::#KRATE::parse::AsyncParse for #name {
                type Fut<'a, R: ::#KRATE::reader::PollReader + Unpin> = #parse_name<'a, R>;
                fn create_fut<'a, R: ::#KRATE::reader::PollReader + Unpin>(reader: R, options: &'a ::#KRATE::parse_options::ParseOptions) -> Self::Fut<'a, R> {
                    #parse_name {
                        inner: <#parse_repr>::create_fut(reader, options)
                    }
                }
            }

            impl <'a, R: ::#KRATE::reader::PollReader + Unpin> ::#KRATE::reader::TakeReader<'a, R> for #parse_name<'a, R> {
                fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                    self.inner.take_reader()
                }
                fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                    self.inner.borrow_reader()
                }
            }
        }
    }
}

impl syn::parse::Parse for FlagList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let content = content.parse_terminated(Flag::parse, syn::Token![;])?;
        let flags = content.into_iter().collect::<Vec<_>>();
        Ok(Self {
            flags,
        })
    }
}

