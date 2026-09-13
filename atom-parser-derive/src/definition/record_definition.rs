use crate::{FourCC, AtomField, AtomFields};

pub struct AtomDefinition {
    pub fcc: FourCC,
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub fields: Vec<AtomField>,
}

impl AtomDefinition {
    pub fn parse_from_syn(input: syn::parse::ParseStream, fcc: FourCC, attrs: Vec<syn::Attribute>, name: syn::Ident) -> syn::Result<Self> {
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
    pub fn parse_from_syn(input: syn::parse::ParseStream, attrs: Vec<syn::Attribute>, name: syn::Ident) -> syn::Result<Self> {
        let mut generics = input.parse::<syn::Generics>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![where]) {
            generics.where_clause = Some(input.parse()?);
        }
        let fields = input.parse::<AtomFields>()?.inner;

        if fields.iter().any(|field| matches!(field, AtomField::FullBox {..} | AtomField::Children { .. })) {
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
        let name = self.name.to_string();

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
}
