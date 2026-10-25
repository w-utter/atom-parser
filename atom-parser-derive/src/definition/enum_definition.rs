pub struct EnumDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub variants: Vec<EnumVariant>,
    pub repr: syn::Type,
}

impl EnumDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        attrs: Vec<syn::Attribute>,
        repr: syn::Type,
    ) -> syn::Result<Self> {
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

pub struct EnumVariantList {
    pub variants: Vec<EnumVariant>,
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
