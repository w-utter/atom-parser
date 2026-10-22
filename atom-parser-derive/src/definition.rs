use crate::FourCC;

mod enum_definition;
pub use enum_definition::EnumDefinition;
mod record_definition;
pub use record_definition::{AtomDefinition, StructDefinition};
mod newtype;
pub use newtype::NewTypeDefinition;

pub struct Definition {
    pub version: Option<syn::LitInt>,
    pub kind: DefinitionKind,
}

impl syn::parse::Parse for Definition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;

        let mut fcc = attrs
            .iter()
            .enumerate()
            .filter(|(_, attr)| attr.path().is_ident("atom"));

        if fcc.clone().count() > 1 {
            panic!("more than 1 atom attr specified")
        }

        let fcc = fcc
            .next()
            .map(|(pos, _)| pos)
            .map(|pos| attrs.swap_remove(pos))
            .map(|attr| FourCC::from_syn(attr))
            .transpose()?;

        let mut version = attrs
            .iter()
            .enumerate()
            .filter(|(_, attr)| attr.path().is_ident("version"));

        if version.clone().count() > 1 {
            panic!("more than 1 version");
        }

        let version = version
            .next()
            .map(|(pos, _)| pos)
            .map(|pos| attrs.swap_remove(pos))
            .map(|attr| attr.parse_args::<syn::LitInt>())
            .transpose()?;

        let _: syn::Visibility = input.parse()?;

        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![struct]) {
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;

            let mut generics = input.parse::<syn::Generics>()?;
            let lookahead = input.lookahead1();
            if lookahead.peek(syn::Token![where]) {
                generics.where_clause = Some(input.parse()?);
            }

            let lookahead = input.lookahead1();

            let kind = match (lookahead.peek(syn::token::Paren), fcc) {
                (false, Some(fcc)) => {
                    if generics != Default::default() {
                        panic!("generics are not supported for atoms")
                    }
                    DefinitionKind::Atom(AtomDefinition::parse_from_syn(input, fcc, attrs, name)?)
                }
                (false, None) => DefinitionKind::Struct(StructDefinition::parse_from_syn(
                    input, attrs, name, generics,
                )?),
                (true, None) => DefinitionKind::NewType(NewTypeDefinition::parse_from_syn(
                    input, attrs, name, generics,
                )?),
                (true, Some(_)) => panic!("fourcc specified for newtype"),
            };

            Ok(Self { version, kind })
        } else if lookahead.peek(syn::Token![enum]) {
            let _: syn::Token![enum] = input.parse()?;
            if version.is_some() || fcc.is_some() {
                panic!("veresion/atom attr is not supported for enums");
            }

            let kind = EnumDefinition::parse_from_syn(input, attrs)?;
            Ok(Self {
                version: None,
                kind: DefinitionKind::Enum(kind),
            })
        } else {
            panic!("unknown layout")
        }
    }
}

pub enum DefinitionKind {
    Struct(StructDefinition),
    Atom(AtomDefinition),
    NewType(NewTypeDefinition),
    Enum(EnumDefinition),
}

pub struct DefinitionList {
    pub defs: Vec<Definition>,
}

impl syn::parse::Parse for DefinitionList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut defs = vec![];
        while !input.is_empty() {
            defs.push(input.parse()?);
        }
        Ok(DefinitionList { defs })
    }
}
