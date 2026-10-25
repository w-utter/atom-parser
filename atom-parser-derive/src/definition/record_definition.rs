use crate::{AtomField, AtomFields, FourCC};

pub struct AtomDefinition {
    pub fcc: FourCC,
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub fields: Vec<AtomField>,
}

impl AtomDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        fcc: FourCC,
        attrs: Vec<syn::Attribute>,
        name: syn::Ident,
    ) -> syn::Result<Self> {
        let fields = input.parse::<AtomFields>()?.inner;
        Ok(Self {
            fcc,
            attrs,
            name,
            fields,
        })
    }
}

pub struct StructDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub fields: Vec<AtomField>,
}

impl StructDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        attrs: Vec<syn::Attribute>,
        name: syn::Ident,
        generics: syn::Generics,
    ) -> syn::Result<Self> {
        let fields = input.parse::<AtomFields>()?.inner;

        if fields.iter().any(|field| {
            matches!(
                field,
                AtomField::FullBox { .. } | AtomField::Children { .. }
            )
        }) {
            panic!("unsupported field in non-atom definition");
        }

        Ok(Self {
            attrs,
            name,
            generics,
            fields,
        })
    }

    pub fn module_name(&self) -> syn::Ident {
        super::ident_to_snake_case(&self.name)
    }
}
