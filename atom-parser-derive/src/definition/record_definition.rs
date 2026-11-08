use crate::{AtomField, AtomFieldKind, AtomFields, FourCC, field::ChildrenKind};

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

#[derive(Debug)]
pub struct Tag {
    pub ty: syn::Type,
    pub expr: syn::Expr,
}

pub struct StructDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub tag: Option<Tag>,
    pub generics: syn::Generics,
    pub fields: Vec<AtomField>,
}

impl StructDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        attrs: Vec<syn::Attribute>,
        name: syn::Ident,
        tag: Option<Tag>,
        generics: syn::Generics,
    ) -> syn::Result<Self> {
        let fields = input.parse::<AtomFields>()?.inner;

        if fields.iter().any(|field| {
            matches!(
                field.kind,
                AtomFieldKind::FullBox { .. }
                    | AtomFieldKind::Children {
                        kind: ChildrenKind::Atom,
                        ..
                    }
            )
        }) {
            panic!("unsupported field in non-atom definition");
        }

        Ok(Self {
            attrs,
            name,
            generics,
            fields,
            tag,
        })
    }

    pub fn module_name(&self) -> syn::Ident {
        super::ident_to_snake_case(&self.name)
    }
}
