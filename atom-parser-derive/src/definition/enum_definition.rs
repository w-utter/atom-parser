pub struct EnumDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub variants: Vec<EnumVariant>,
    pub repr: syn::Type,
}

impl EnumDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        mut attrs: Vec<syn::Attribute>,
    ) -> syn::Result<Self> {
        let mut repr = attrs
            .iter()
            .enumerate()
            .filter(|(_, attr)| attr.path().is_ident("enum_repr"));

        if repr.clone().count() > 1 {
            panic!("more than 1 repr attr specified");
        }

        let repr = repr
            .next()
            .map(|(pos, _)| pos)
            .map(|pos| attrs.swap_remove(pos))
            .map(|attr| attr.parse_args::<syn::Type>())
            .transpose()?
            .expect("no size attr specified for enum");

        let name = input.parse()?;
        let variants = input.parse::<EnumVariantList>()?.variants;
        Ok(Self {
            repr,
            name,
            variants,
            attrs,
        })
    }
}

struct EnumVariantList {
    variants: Vec<EnumVariant>,
}

impl syn::parse::Parse for EnumVariantList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let content = content.parse_terminated(EnumVariant::parse, syn::Token![,])?;
        let variants = content.into_iter().collect::<Vec<_>>();
        Ok(Self { variants })
    }
}

pub struct EnumVariant {
    pub name: syn::Ident,
    pub val: syn::Expr,
}

impl syn::parse::Parse for EnumVariant {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _: syn::Token![=] = input.parse()?;
        let val = input.parse()?;

        Ok(Self { name, val })
    }
}
