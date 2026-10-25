use crate::FourCC;

mod enum_definition;
pub use enum_definition::EnumDefinition;
mod record_definition;
pub use record_definition::{AtomDefinition, StructDefinition};
mod newtype;
pub use newtype::NewTypeDefinition;
mod bitwise;
pub use bitwise::{BitFieldDefinition, BitFieldList, FlagsDefinition};

pub struct Definition {
    pub version: Option<syn::LitInt>,
    pub kind: DefinitionKind,
}

fn singular_attr<T>(
    attrs: &mut Vec<syn::Attribute>,
    name: &str,
    f: impl FnOnce(syn::Attribute) -> syn::Result<T>,
) -> syn::Result<Option<T>> {
    let mut attr = attrs
        .iter()
        .enumerate()
        .filter(|(_, attr)| attr.path().is_ident(name));

    if attr.clone().count() > 1 {
        panic!("more than 1 {name} attr specified")
    }

    attr.next()
        .map(|(pos, _)| pos)
        .map(|pos| attrs.swap_remove(pos))
        .map(f)
        .transpose()
}

impl syn::parse::Parse for Definition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;

        let fcc = singular_attr(&mut attrs, "atom", FourCC::from_syn)?;
        let version = singular_attr(&mut attrs, "version", |attr| {
            attr.parse_args::<syn::LitInt>()
        })?;
        let bitfield_repr = singular_attr(&mut attrs, "bitfield_repr", |attr| {
            attr.parse_args::<syn::Type>()
        })?;
        let flags_repr = singular_attr(&mut attrs, "flags_repr", |attr| {
            attr.parse_args::<syn::Type>()
        })?;
        let enum_repr = singular_attr(&mut attrs, "enum_repr", |attr| {
            attr.parse_args::<syn::Type>()
        })?;

        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![struct]) {
            if enum_repr.is_some() {
                panic!("enum_repr specified for struct")
            }

            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;

            let mut generics = input.parse::<syn::Generics>()?;
            let lookahead = input.lookahead1();
            if lookahead.peek(syn::Token![where]) {
                generics.where_clause = Some(input.parse()?);
            }

            let lookahead = input.lookahead1();

            let kind = match (
                lookahead.peek(syn::token::Paren),
                fcc,
                flags_repr,
                bitfield_repr,
            ) {
                (false, Some(fcc), None, None) => {
                    if generics != Default::default() {
                        panic!("generics are not supported for atoms")
                    }
                    DefinitionKind::Atom(AtomDefinition::parse_from_syn(input, fcc, attrs, name)?)
                }
                (false, None, Some(flags), None) => DefinitionKind::Flags(
                    FlagsDefinition::parse_from_syn(input, attrs, name, flags)?,
                ),
                (false, None, None, Some(bitfield)) => DefinitionKind::BitField(
                    BitFieldDefinition::parse_from_syn(input, attrs, name, bitfield)?,
                ),
                (false, None, None, None) => DefinitionKind::Struct(
                    StructDefinition::parse_from_syn(input, attrs, name, generics)?,
                ),
                (true, None, None, None) => DefinitionKind::NewType(
                    NewTypeDefinition::parse_from_syn(input, attrs, name, generics)?,
                ),
                (a, b, c, d) => panic!(
                    "invalid configuration( newtype: {a:?}, fourcc: {b:?}, flags: {c:?}, bitfield: {d:?})"
                ),
            };

            Ok(Self { version, kind })
        } else if lookahead.peek(syn::Token![enum]) {
            let _: syn::Token![enum] = input.parse()?;
            if version.is_some() || fcc.is_some() {
                panic!("veresion/atom attr is not supported for enums");
            } else if bitfield_repr.is_some() || flags_repr.is_some() {
                panic!("multiple specified reprs");
            }

            let repr = enum_repr.expect("no enum_repr specified for enum");
            let kind = EnumDefinition::parse_from_syn(input, attrs, repr)?;
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
    Flags(FlagsDefinition),
    BitField(BitFieldDefinition),
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

pub(crate) fn ident_to_snake_case(ident: &syn::Ident) -> syn::Ident {
    let name = ident.to_string();

    let mut snake = String::default();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(ch.to_ascii_lowercase());
        } else {
            snake.push(ch);
        }
    }
    syn::Ident::new(&snake, proc_macro2::Span::call_site())
}
