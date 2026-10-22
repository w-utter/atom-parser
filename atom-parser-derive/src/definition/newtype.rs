pub struct NewTypeDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub tys: Vec<syn::Type>,
}

impl NewTypeDefinition {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        attrs: Vec<syn::Attribute>,
        name: syn::Ident,
        generics: syn::Generics,
    ) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        use syn::parse::Parse;
        let content = content.parse_terminated(syn::Type::parse, syn::Token![,])?;
        let tys = content.into_iter().collect::<Vec<_>>();
        let _: syn::Token![;] = input.parse()?;
        Ok(Self {
            attrs,
            name,
            generics,
            tys,
        })
    }
}
