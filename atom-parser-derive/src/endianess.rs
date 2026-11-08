use crate::KRATE;

#[derive(Clone, Copy)]
pub enum Endianess {
    Big,
    Little,
    // use whatever the user specified in the parseoptions
    UserDefined,
}

impl Endianess {
    pub fn parse_from_syn(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let path: syn::Path = input.parse()?;
        Ok(
            match path.require_ident()?.to_string().to_lowercase().as_str() {
                "big" => Self::Big,
                "little" => Self::Little,
                "user" | "user_defined" | "user-defined" => Self::UserDefined,
                o => panic!("unknown endianess: {o:?}"),
            },
        )
    }

    pub fn format_options_ty(
        options: Option<&Self>,
        options_ty: Option<proc_macro2::TokenStream>,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        let options_ty = options_ty.unwrap_or(quote!(__Options));
        match options {
            None => options_ty,
            Some(Endianess::Little) => {
                quote!(::#KRATE::parse_options::WithLittleEndian<#options_ty>)
            }
            Some(Endianess::Big) => quote!(::#KRATE::parse_options::WithBigEndian<#options_ty>),
            Some(Endianess::UserDefined) => {
                quote!(::#KRATE::parse_options::WithUserEndianess<#options_ty>)
            }
        }
    }

    pub fn format_options(
        options: Option<&Self>,
        nested: Option<proc_macro2::TokenStream>,
        sync: bool,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        let creation = Self::format_options_create_fut(options, nested, sync);
        quote!(let opts = #creation;)
    }

    pub fn format_options_create_fut(
        options: Option<&Self>,
        nested: Option<proc_macro2::TokenStream>,
        sync: bool,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        let opts = nested.unwrap_or(if sync { quote!(&opts) } else { quote!(opts) });
        match options {
            None => quote!(#opts),
            Some(Endianess::Little) => {
                quote!(::#KRATE::parse_options::WithLittleEndian { opts: #opts })
            }
            Some(Endianess::Big) => quote!(::#KRATE::parse_options::WithBigEndian { opts: #opts }),
            Some(Endianess::UserDefined) => {
                quote!(::#KRATE::parse_options::WithUserEndianess { opts: #opts })
            }
        }
    }

    pub fn format_state_transition_options(
        field_endianess: Option<&Self>,
        next_field_endianess: Option<&Self>,
        definition_endianess: Option<&Self>,
        first: bool,
    ) -> proc_macro2::TokenStream {
        let take = if first {
            None
        } else {
            Self::get_options_inner(definition_endianess, field_endianess)
        };
        Self::format_options_create_fut(
            next_field_endianess,
            Some(Self::format_options_create_fut(
                definition_endianess,
                take,
                false,
            )),
            false,
        )
    }

    pub fn get_options_inner(
        definition_endianess: Option<&Self>,
        field_endianess: Option<&Self>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        match (field_endianess.is_some(), definition_endianess.is_some()) {
            (true, true) => Some(quote!(opts.opts.opts)),
            (true, false) | (false, true) => Some(quote!(opts.opts)),
            (false, false) => None,
        }
    }

    pub fn format_take_reader(
        endianess: Option<&Self>,
        field_endianess: Option<&Self>,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        Self::get_options_inner(endianess, field_endianess).unwrap_or(quote!(opts))
    }

    pub fn format_borrow_reader(
        endianess: Option<&Self>,
        field_endianess: Option<&Self>,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        let take = Self::format_take_reader(endianess, field_endianess);
        quote!(&#take)
    }
}
