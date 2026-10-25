use crate::{
    KRATE,
    definition::enum_definition::{EnumVariant, EnumVariantList},
};

pub struct FlagsDefinition {
    pub name: syn::Ident,
    pub parse_repr: syn::Type,
    pub flags: Vec<crate::Flag>,
    pub attrs: Vec<syn::Attribute>,
}

impl FlagsDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        attrs: Vec<syn::Attribute>,
        name: syn::Ident,
        parse_repr: syn::Type,
    ) -> syn::Result<Self> {
        let flags = input.parse::<crate::FlagList>()?.flags;
        Ok(Self {
            attrs,
            name,
            parse_repr,
            flags,
        })
    }
}

pub enum BitFieldKind {
    Normal(syn::Type),
    Reserved,
    Flag,
    Enum {
        name: syn::Ident,
        repr: syn::Type,
        variants: Vec<EnumVariant>,
    },
}

pub struct BitField {
    pub name: syn::Ident,
    pub bits: usize,
    pub kind: BitFieldKind,
}

impl syn::parse::Parse for BitField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        if attrs.len() > 2 {
            panic!("more than 2 attrs specified for flag")
        }

        enum Kind {
            Reserved,
            Flag,
            Enum { signed: bool },
        }

        let mut kind = None;
        let mut bits = None;

        for attr in attrs.into_iter() {
            let path = attr.path();
            if path.is_ident("reserved") {
                if kind.is_some() {
                    panic!("duplicate reserved attrs")
                }
                kind = Some(Kind::Reserved)
            } else if path.is_ident("bits") {
                if bits.is_some() {
                    panic!("duplicate bits/flag attrs")
                }
                let size = attr.parse_args::<syn::LitInt>()?.base10_parse::<usize>()?;
                bits = Some(size)
            } else if path.is_ident("enum_bits") {
                if bits.is_some() || kind.is_some() {
                    panic!("duplicate bits/flag attrs for enum")
                }

                let (size, signed) = attr.parse_args_with(|input: syn::parse::ParseStream| {
                    let size = input.parse::<syn::LitInt>()?.base10_parse::<usize>()?;
                    let mut signed = false;
                    if let Some(_) = input.parse::<Option<syn::Token![,]>>()? {
                        let name: syn::Path = input.parse()?;
                        if !name.is_ident("signed") {
                            panic!(
                                "unknown second arg for enum: {:?}, only `signed` supported",
                                name
                            );
                        }
                        signed = true;
                    }
                    Ok((size, signed))
                })?;

                bits = Some(size);
                kind = Some(Kind::Enum { signed });
            } else if path.is_ident("flag") {
                if bits.is_some() || kind.is_some() {
                    panic!("duplicate bits/flag attrs")
                }
                bits = Some(1);
                kind = Some(Kind::Flag);
            } else {
                panic!("unknown attr for field")
            }
        }

        let name = input.parse()?;
        let _: syn::Token![:] = input.parse()?;

        let bits = bits.expect("no bit width specified for bitfield");
        let kind = match kind {
            Some(Kind::Enum { signed }) => {
                let _: syn::Token![enum] = input.parse()?;
                let enum_name = input.parse()?;
                let variants = input.parse::<EnumVariantList>()?.variants;

                let repr = if bits <= 8 {
                    if signed {
                        syn::parse_quote!(i8)
                    } else {
                        syn::parse_quote!(u8)
                    }
                } else if bits <= 16 {
                    if signed {
                        syn::parse_quote!(i16)
                    } else {
                        syn::parse_quote!(u16)
                    }
                } else if bits <= 32 {
                    if signed {
                        syn::parse_quote!(i32)
                    } else {
                        syn::parse_quote!(u32)
                    }
                } else if bits <= 64 {
                    if signed {
                        syn::parse_quote!(i64)
                    } else {
                        syn::parse_quote!(u64)
                    }
                } else {
                    panic!("nested enum with more than 64 bits is not supported")
                };

                BitFieldKind::Enum {
                    name: enum_name,
                    variants,
                    repr,
                }
            }
            k => {
                let ty = input.parse()?;
                match k {
                    Some(Kind::Reserved) => BitFieldKind::Reserved,
                    Some(Kind::Flag) => BitFieldKind::Flag,
                    None => BitFieldKind::Normal(ty),
                    Some(Kind::Enum { .. }) => unreachable!(),
                }
            }
        };

        Ok(Self { name, kind, bits })
    }
}

pub struct BitFieldList {
    fields: Vec<BitField>,
}

impl syn::parse::Parse for BitFieldList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let content = content.parse_terminated(BitField::parse, syn::Token![,])?;
        let fields = content.into_iter().collect::<Vec<_>>();
        Ok(Self { fields })
    }
}

impl BitFieldList {
    fn mask_from_width_and_offset(bit_width: usize, offset: usize) -> syn::Index {
        syn::Index::from(((1 << bit_width) - 1) << offset)
    }

    pub fn check_reserved_fields(fields: &[BitField]) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mut offset = 0;
        let reserved_ranges = fields
            .iter()
            .filter_map(|field| {
                let res = if matches!(field.kind, BitFieldKind::Reserved) {
                    let mask = Self::mask_from_width_and_offset(field.bits, offset);
                    Some(quote!(#mask))
                } else {
                    None
                };
                offset += field.bits;
                res
            })
            .collect::<Vec<_>>();

        if !reserved_ranges.is_empty() {
            Some(quote! {
                if (opts.error_on_unknown_flags && ((bits & (#((#reserved_ranges))|*))) != 0) {
                    return Err(::#KRATE::error::ParseError::UnknownFlags);
                }
            })
        } else {
            None
        }
    }

    pub fn inline_definitions(
        fields: &[BitField],
        mod_name: &syn::Ident,
        attrs: &[syn::Attribute],
    ) -> proc_macro2::TokenStream {
        use quote::quote;

        let definitions = fields.iter().filter_map(|field| {
            match &field.kind {
                BitFieldKind::Enum { name: enum_name, repr, variants,} => {
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
                            Self::#variant => Self::#name
                        }
                    }).collect::<Vec<_>>();

                    let try_into_impl = variants.iter().enumerate().map(|(i, v)| {
                        let variant = quote::format_ident!("VARIANT_{}", i);
                        let name = &v.name;
                        quote!{
                            Self::#name => Self::#variant
                        }
                    });

                    Some(quote! {
                        #(#attrs)*
                        #[derive(Debug)]
                        pub enum #enum_name {
                            #(#enum_variants,)*
                        }

                        impl #enum_name {
                            #(#const_bindings)*
                        }

                        impl ::#KRATE::bits::Bits for #enum_name {
                            type Repr = #repr;
                            fn from_bits(repr: Self::Repr) -> Self {
                                match repr {
                                    #(#try_from_impl,)*
                                    u => panic!("unknown enum variant bits for {}: {}", stringify!(#enum_name), u)
                                }
                            }
                            fn to_bits(self) -> Self::Repr {
                                match self {
                                    #(#try_into_impl,)*
                                }
                            }
                        }
                    })
                }
                _ => None,
            }
        });

        quote! {
            pub mod #mod_name {
                #(#definitions)*
            }
        }
    }

    pub fn debug_impl(fields: &[BitField], name: &syn::Ident) -> proc_macro2::TokenStream {
        use quote::quote;
        let debug_fields = fields.iter().filter_map(|field| match &field.kind {
            BitFieldKind::Reserved => None,
            BitFieldKind::Normal(_) | BitFieldKind::Flag | BitFieldKind::Enum { .. } => {
                let field_name = &field.name;

                Some(quote! {
                    .field(stringify!(#field_name), &self.#field_name())
                })
            }
        });

        quote! {
            impl ::core::fmt::Debug for #name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct(stringify!(#name))
                        #(#debug_fields)*
                        .finish()
                }
            }
        }
    }

    pub fn field_helpers(
        fields: &[BitField],
        name: &syn::Ident,
        mod_name: &syn::Ident,
    ) -> proc_macro2::TokenStream {
        use quote::quote;

        let mut offset = 0;
        let helpers = fields.iter().filter_map(|field| {
            let mask = Self::mask_from_width_and_offset(field.bits, offset);
            let field_name = &field.name;
            let helper = match &field.kind {
                BitFieldKind::Normal(ty) => {
                    let shift = syn::Index::from(offset);
                    Some(quote!{
                        pub fn #field_name(&self) -> #ty {
                            let val = ((self.0) & #mask) >> #shift;
                            <#ty as ::#KRATE::bits::Bits>::from_bits(val as <#ty as ::#KRATE::bits::Bits>::Repr)
                        }
                    })
                }
                BitFieldKind::Flag => {
                    let shift = syn::Index::from(offset);
                    Some(quote! {
                        pub fn #field_name(&self) -> bool {
                            let val = ((self.0) & #mask) >> #shift;
                            val > 0
                        }
                    })
                }
                BitFieldKind::Enum { name: enum_name, repr, .. } => {
                    let shift = syn::Index::from(offset);
                    Some(quote!{
                        pub fn #field_name(&self) -> #mod_name :: #enum_name {
                            let val = ((self.0) & #mask) >> #shift;
                            <#mod_name :: #enum_name as ::#KRATE::bits::Bits>::from_bits(val as #repr)
                        }
                    })
                }
                BitFieldKind::Reserved => None,
            };
            offset += field.bits;
            helper
        });

        quote! {
            impl #name {
                #(#helpers)*
            }
        }
    }
}

pub struct BitFieldDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub parse_repr: syn::Type,
    pub fields: Vec<BitField>,
}

impl BitFieldDefinition {
    pub fn module_name(&self) -> syn::Ident {
        super::ident_to_snake_case(&self.name)
    }
}

impl BitFieldDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        attrs: Vec<syn::Attribute>,
        name: syn::Ident,
        parse_repr: syn::Type,
    ) -> syn::Result<Self> {
        let fields = input.parse::<BitFieldList>()?.fields;
        Ok(Self {
            attrs,
            name,
            parse_repr,
            fields,
        })
    }
}
