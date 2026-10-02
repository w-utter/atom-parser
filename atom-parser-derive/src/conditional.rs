use crate::{AtomField, AtomFields, KRATE, StructFieldAttr};

#[derive(Clone)]
pub enum Condition {
    Match {
        expr: syn::Expr,
        arms: Vec<MatchArm>,
    },
    Ifs {
        branches: Vec<Branch>,
    },
}

impl Condition {
    pub fn parse_from_syn(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        Ok(if lookahead.peek(syn::Token![if]) {
            Condition::parse_ifs_from_syn(input)?
        } else if lookahead.peek(syn::Token![match]) {
            Condition::parse_match_from_syn(input)?
        } else {
            panic!("unknown conditional")
        })
    }

    pub fn parse_match_from_syn(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let _: syn::Token![match] = input.parse()?;
        let expr = syn::Expr::parse_without_eager_brace(input)?;
        let content;
        syn::braced!(content in input);
        let mut arms = vec![];

        while !content.is_empty() {
            arms.push(MatchArm::parse_from_syn(&content)?);
        }

        Ok(Self::Match { expr, arms })
    }

    pub fn parse_ifs_from_syn(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut branches = vec![];

        let _: syn::Token![if] = input.parse()?;
        let cond = input.call(syn::Expr::parse_without_eager_brace)?;
        let content;
        syn::braced!(content in input);
        let result = ConditionalResult::parse_from_syn(&content)?;
        branches.push(Branch {
            kind: BranchKind::If(cond),
            result,
        });

        while !input.is_empty() {
            let _: syn::Token![else] = input.parse()?;
            let lookahead = input.lookahead1();

            let kind = if lookahead.peek(syn::Token![if]) {
                let _: syn::Token![if] = input.parse()?;
                let cond = input.call(syn::Expr::parse_without_eager_brace)?;
                BranchKind::ElseIf(cond)
            } else {
                BranchKind::Else
            };

            let content;
            syn::braced!(content in input);
            let result = ConditionalResult::parse_from_syn(&content)?;

            branches.push(Branch { kind, result });
        }

        Ok(Self::Ifs { branches })
    }

    pub fn is_exhaustive(&self) -> bool {
        match self {
            Self::Match { arms, .. } => arms
                .iter()
                .filter_map(|arm| {
                    if let ConditionalResult::NestedConditional(cond) = &arm.body {
                        Some(cond)
                    } else {
                        None
                    }
                })
                .all(|cond| cond.is_exhaustive()),
            Self::Ifs { branches } => {
                branches
                    .iter()
                    .filter_map(|branch| {
                        if let ConditionalResult::NestedConditional(cond) = &branch.result {
                            Some(cond)
                        } else {
                            None
                        }
                    })
                    .all(|cond| cond.is_exhaustive())
                    && matches!(
                        branches.last().unwrap(),
                        Branch {
                            kind: BranchKind::Else,
                            ..
                        }
                    )
            }
        }
    }

    pub fn format_conditional(
        &self,
        f: &mut impl FnMut(&syn::Ident, &[AtomField], bool) -> proc_macro2::TokenStream,
        on_conditional: &mut impl FnMut() -> proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        match self {
            Self::Match { expr, arms } => {
                let arms = arms.iter().map(|arm| {
                    let pat = &arm.pat;
                    let body = arm.body.format_conditional(f, on_conditional, true);
                    quote!(
                        #pat => {
                            #body
                        }
                    )
                });
                quote! {
                    match #expr {
                        #(#arms)*
                    }
                }
            }
            Self::Ifs { branches } => {
                let exhaustive = matches!(
                    branches.last().unwrap(),
                    Branch {
                        kind: BranchKind::Else,
                        ..
                    }
                );
                let else_branch = if !exhaustive {
                    let none = on_conditional();
                    Some(quote! {
                        else {
                            #none
                        }
                    })
                } else {
                    None
                };

                let branches = branches.iter().map(|branch| {
                    let cond = match &branch.kind {
                        BranchKind::If(e) => quote!(if #e),
                        BranchKind::ElseIf(e) => quote!(else if #e),
                        BranchKind::Else => quote!(else),
                    };

                    let body = branch
                        .result
                        .format_conditional(f, on_conditional, exhaustive);

                    quote! {
                        #cond {
                            #body
                        }
                    }
                });
                quote! {
                    #(#branches)*
                    #else_branch
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct MatchArm {
    pub pat: syn::Pat,
    pub body: ConditionalResult,
}

// copied from https://docs.rs/syn/latest/syn/enum.Pat.html
// `Pat::parse_multi_with_leading_vert_and_guard` is pub(crate) for whatever reason
mod syn_impl {
    pub(super) fn parse_match_pattern_multi_with_leading_vert_and_guard(
        input: syn::parse::ParseStream,
    ) -> syn::Result<syn::Pat> {
        let leading_vert: Option<syn::Token![|]> = input.parse()?;
        let allow_guard = true;
        multi_pat_impl(input, leading_vert, allow_guard)
    }

    fn multi_pat_impl(
        input: syn::parse::ParseStream,
        leading_vert: Option<syn::Token![|]>,
        allow_guard: bool,
    ) -> syn::Result<syn::Pat> {
        let mut pat = syn::Pat::parse_single(input)?;
        if leading_vert.is_some()
            || input.peek(syn::Token![|])
                && !input.peek(syn::Token![||])
                && !input.peek(syn::Token![|=])
        {
            let mut cases = syn::punctuated::Punctuated::new();
            cases.push_value(pat);
            while input.peek(syn::Token![|])
                && !input.peek(syn::Token![||])
                && !input.peek(syn::Token![|=])
            {
                let punct = input.parse()?;
                cases.push_punct(punct);
                let pat = syn::Pat::parse_single(input)?;
                cases.push_value(pat);
            }
            pat = syn::Pat::Or(syn::PatOr {
                attrs: Vec::new(),
                leading_vert,
                cases,
            });
        }
        if allow_guard && input.peek(syn::Token![if]) {
            let if_token: syn::Token![if] = input.parse()?;
            let guard: syn::Expr = input.parse()?;
            pat = syn::Pat::Guard(syn::PatGuard {
                attrs: Vec::new(),
                pat: Box::new(pat),
                if_token,
                guard: Box::new(guard),
            });
        }
        Ok(pat)
    }
}

impl MatchArm {
    fn parse_from_syn(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let pat = syn_impl::parse_match_pattern_multi_with_leading_vert_and_guard(input)?;
        let _: syn::Token![=>] = input.parse()?;

        let mut input = input;
        let lookahead = input.lookahead1();
        let mut needs_comma = true;
        let content;
        if lookahead.peek(syn::token::Brace) {
            syn::braced!(content in input);
            input = &content;
            needs_comma = false;
        } else if lookahead.peek(syn::Token![if]) || lookahead.peek(syn::Token![match]) {
            needs_comma = false;
        }

        if needs_comma {
            let _: syn::Token![,] = input.parse()?;
        }

        let body = ConditionalResult::parse_from_syn(input)?;
        Ok(Self { pat, body })
    }
}

#[derive(Clone)]
pub struct Conditionals {
    pub name: Option<syn::Ident>,

    pub field_name: syn::Ident,
    pub condition: Condition,
}

#[derive(Clone)]
pub enum BranchKind {
    If(syn::Expr),
    ElseIf(syn::Expr),
    Else,
}

#[derive(Clone)]
pub struct Branch {
    pub kind: BranchKind,
    pub result: ConditionalResult,
}

// conditions can only contain either another nested condition or a variant,
// so that no other intermediate state needs to be stored
#[derive(Clone)]
pub enum ConditionalResult {
    Variant {
        name: syn::Ident,
        fields: Vec<AtomField>,
    },
    NestedConditional(Condition),
}

impl ConditionalResult {
    fn parse_from_syn(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![if]) || lookahead.peek(syn::Token![match]) {
            return Ok(ConditionalResult::NestedConditional(
                Condition::parse_from_syn(input)?,
            ));
        }

        let name = input.parse()?;
        let fields = input.parse::<AtomFields>()?.inner;

        Ok(Self::Variant { name, fields })
    }

    fn append_grouped(&self, grouped: &mut Vec<ConditionalVariant>) {
        match self {
            Self::Variant { name, fields } => {
                grouped.push(ConditionalVariant {
                    variant_name: name.clone(),
                    fields: fields.clone(),
                });
            }
            Self::NestedConditional(Condition::Match { arms, .. }) => {
                for arm in arms {
                    arm.body.append_grouped(grouped)
                }
            }
            Self::NestedConditional(Condition::Ifs { branches }) => {
                for branch in branches {
                    branch.result.append_grouped(grouped)
                }
            }
        }
    }

    pub fn format_conditional(
        &self,
        f: &mut impl FnMut(&syn::Ident, &[AtomField], bool) -> proc_macro2::TokenStream,
        on_conditional: &mut impl FnMut() -> proc_macro2::TokenStream,
        exhaustive: bool,
    ) -> proc_macro2::TokenStream {
        match self {
            Self::Variant { name, fields } => f(&name, &fields, exhaustive),
            Self::NestedConditional(cond) => cond.format_conditional(f, on_conditional),
        }
    }
}

pub enum GroupedConditionals {
    Struct {
        name: syn::Ident,
        fields: Vec<AtomField>,
    },
    Enum {
        name: syn::Ident,
        variants: Vec<ConditionalVariant>,
    },
}

pub struct ConditionalVariant {
    pub variant_name: syn::Ident,
    pub fields: Vec<AtomField>,
}

impl ConditionalVariant {
    fn format_variant_parse(
        vname: &syn::Ident,
        exhaustive: bool,
        parse_ty_generics: &syn::TypeGenerics,
    ) -> proc_macro2::TokenStream {
        let name = if !exhaustive {
            quote::format_ident!("Some{}", vname)
        } else {
            vname.clone()
        };

        let parse_name = quote::format_ident!("Async{}Parse", vname);

        quote::quote! {
            #name (#parse_name #parse_ty_generics),
        }
        .into()
    }

    fn format_variant_storage(
        vname: &syn::Ident,
        exhaustive: bool,
        storage_ty_generics: &syn::TypeGenerics,
    ) -> proc_macro2::TokenStream {
        let name = if !exhaustive {
            quote::format_ident!("Some{}", vname)
        } else {
            vname.clone()
        };

        let storage_name = quote::format_ident!("Async{}Storage", vname);

        quote::quote! {
            #name (#storage_name #storage_ty_generics),
        }
        .into()
    }

    fn format_variant_statemachine(
        vname: &syn::Ident,
        fields: &[AtomField],
        generics: &syn::Generics,
        generic_collection: Option<&proc_macro2::TokenStream>,
        parse_generics: &syn::Generics,
        storage_generics: &syn::Generics,
        payload_len: Option<&syn::Type>,
    ) -> proc_macro2::TokenStream {
        let (parse_impl_generics, parse_ty_generics, parse_where_clause) =
            parse_generics.split_for_impl();

        let typed_fields = crate::field::prepare_fields_for_statemachine(
            fields,
            generics.clone(),
            None,
            payload_len,
        );

        let parse_name = quote::format_ident!("Async{}Parse", vname);

        let (state_machine_variants, _state_machine_variant_parsing, borrow_reader_variants) =
            crate::field::format_statemachine_fields(
                fields,
                &typed_fields,
                payload_len,
                vname,
                &parse_name,
                &generics,
                generic_collection,
                None,
                &|_| Default::default(),
                &|_| Default::default(),
                false,
            );

        let storage_fields = fields
            .iter()
            .filter_map(|f| f.as_field_decl(None, payload_len, generics));

        let storage_name = quote::format_ident!("Async{}Storage", vname);
        let (_, nop_variant, _, nop_borrow) = crate::field::format_statemachine_nop_impl(
            &typed_fields,
            &generics,
            None,
            generic_collection,
            payload_len,
            &|_| Default::default(),
        );

        let pd_generics = generics
            .params
            .iter()
            .filter_map(|p| match p {
                syn::GenericParam::Type(t) => Some(&t.ident),
                _ => None,
            })
            .collect::<Vec<_>>();

        quote::quote! {
            pub enum #parse_name #parse_generics {
                #(#state_machine_variants,)*
                #nop_variant
                PdStorage(::core::convert::Infallible, ::core::marker::PhantomData<( O, #(#pd_generics,)* )>),
            }

            impl #parse_impl_generics ::#KRATE::reader::TakeReader<'a, R> for #parse_name #parse_ty_generics #parse_where_clause {
                fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                    unreachable!("reader should not be taken from nested state")
                }
                fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                    match self {
                        #(#borrow_reader_variants)*
                        #nop_borrow
                        Self::PdStorage(..) => unreachable!(),
                    }
                }
            }

            pub struct #storage_name #storage_generics {
                #(#storage_fields,)*
                pub(super) _pd: ::core::marker::PhantomData<( O, #(#pd_generics,)* )>,
            }
        }.into()
    }

    fn format_none_variants(
        exhaustive: bool,
        generics: &syn::Generics,
    ) -> (
        Option<proc_macro2::TokenStream>,
        Option<proc_macro2::TokenStream>,
    ) {
        use quote::quote;
        if !exhaustive {
            let pd_generics = generics.params.iter().filter_map(|p| match p {
                syn::GenericParam::Type(t) => Some(&t.ident),
                _ => None,
            });

            (
                Some(quote! {
                    None(R, &'a ::#KRATE::parse_options::ParseOptions, ::core::marker::PhantomData<(#( #pd_generics,)* )>),
                }),
                Some(quote! {
                    None,
                }),
            )
        } else {
            (None, None)
        }
    }

    fn format_variant_borrow(vname: &syn::Ident, exhaustive: bool) -> proc_macro2::TokenStream {
        let variant_name = if !exhaustive {
            quote::format_ident!("Some{}", vname)
        } else {
            vname.clone()
        };
        quote::quote! {
            Self::#variant_name(v) => v.borrow_reader(),
        }
        .into()
    }

    fn format_none_borrow(exhaustive: bool) -> Option<proc_macro2::TokenStream> {
        if !exhaustive {
            Some(
                quote::quote! {
                    Self::None(reader, opts, _) => (reader, opts),
                }
                .into(),
            )
        } else {
            None
        }
    }

    fn format_field_collection(
        variant_name: &syn::Ident,
        fields: &[AtomField],
        exhaustive: bool,
        mod_name: Option<&proc_macro2::TokenStream>,
        enum_name: &syn::Ident,
        is_struct_definition: bool,
    ) -> proc_macro2::TokenStream {
        use quote::quote;

        let parsed_variant_name = if !exhaustive {
            quote::format_ident!("Some{}", variant_name)
        } else {
            variant_name.clone()
        };

        let parsed_names = fields.iter().filter_map(|f| {
            match f {
                // reused fields are not kept in parsed structs
                AtomField::Struct(_, StructFieldAttr::ReuseCondition) => None,
                field => field.as_collection(),
            }
        });

        let collected_names = fields.iter().filter_map(|f| {
            match f {
                // inline any nested conditional fields
                AtomField::Conditional(cond) => Some(cond.as_async_field_collection(mod_name)),
                field => field.as_collection(),
            }
        });

        let finished_state = if is_struct_definition {
            quote! {
                #mod_name #variant_name {
                    #(#collected_names)*
                    _pd: ::core::marker::PhantomData,
                }
            }
        } else {
            quote! {
                #mod_name #enum_name::#variant_name(#mod_name #variant_name {
                    #(#collected_names)*
                    _pd: ::core::marker::PhantomData,
                })
            }
        };

        let finished_state = if !exhaustive {
            quote! {
                Some(#finished_state)
            }
        } else {
            finished_state
        };

        let storage_enum_name = quote::format_ident!("Async{}Storage", enum_name);
        let storage_variant = quote::format_ident!("Async{}Storage", variant_name);

        quote! {
            #mod_name #storage_enum_name::#parsed_variant_name (#mod_name #storage_variant {
                #(#parsed_names)*
                ..
            }) => {
                #finished_state
            }
        }
    }

    fn format_none_variant_collection(
        exhaustive: bool,
        mod_name: Option<&proc_macro2::TokenStream>,
        enum_name: &syn::Ident,
    ) -> Option<proc_macro2::TokenStream> {
        if !exhaustive {
            let storage_enum_name = quote::format_ident!("Async{}Storage", enum_name);
            Some(
                quote::quote! {
                    #mod_name #storage_enum_name::None => None,
                }
                .into(),
            )
        } else {
            None
        }
    }

    pub fn format_helper_fns(
        name: &syn::Ident,
        fields: &[AtomField],
        generics: &syn::Generics,
    ) -> proc_macro2::TokenStream {
        let mut sync_generics = generics.clone();
        sync_generics
            .params
            .push(syn::parse_quote!(O: ::#KRATE::reader::Offset = usize));

        let mut async_generics = generics.clone();
        async_generics
            .params
            .push(syn::parse_quote!(O: ::#KRATE::reader::Offset + ::core::marker::Unpin = usize));

        let sync_helper_fns = fields.iter().filter_map(|f| f.as_sync_helper_fn(None));
        let async_helper_fns = fields.iter().filter_map(|f| f.as_async_helper_fn(None));
        let (impl_generics, ty_generics, where_clause) = sync_generics.split_for_impl();
        let (async_impl_generics, async_ty_generics, async_where_clause) =
            async_generics.split_for_impl();

        quote::quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#sync_helper_fns)*
            }

            impl #async_impl_generics #name #async_ty_generics #async_where_clause {
                #(#async_helper_fns)*
            }
        }
    }
}

impl Conditionals {
    pub fn parse_from_syn(
        name: Option<syn::Ident>,
        input: syn::parse::ParseStream,
    ) -> syn::Result<Self> {
        let field_name = input.parse()?;
        let _: syn::Token![:] = input.parse()?;
        let content;
        syn::braced!(content in input);
        let condition = Condition::parse_from_syn(&content)?;

        Ok(Self {
            name,
            field_name,
            condition,
        })
    }

    pub fn group(&self) -> syn::Result<GroupedConditionals> {
        let mut variants = vec![];
        match &self.condition {
            Condition::Match { arms, .. } => {
                for arm in arms {
                    arm.body.append_grouped(&mut variants);
                }
            }
            Condition::Ifs { branches } => {
                for branch in branches {
                    branch.result.append_grouped(&mut variants);
                }
            }
        }

        if self.name.is_none() && variants.len() > 1 {
            panic!(
                "if more than 1 variant is specified then a enum name must be provided in the attr"
            )
        }
        Ok(if let Some(name) = self.name.clone() {
            GroupedConditionals::Enum { name, variants }
        } else {
            let ConditionalVariant {
                variant_name,
                fields,
            } = variants.pop().unwrap();
            GroupedConditionals::Struct {
                name: variant_name,
                fields,
            }
        })
    }

    pub fn is_exhaustive(&self) -> bool {
        self.condition.is_exhaustive()
    }

    pub fn format_conditional(
        &self,
        mut f: impl FnMut(&syn::Ident, &[AtomField], bool) -> proc_macro2::TokenStream,
        mut on_conditional: impl FnMut() -> proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        self.condition
            .format_conditional(&mut f, &mut on_conditional)
    }

    pub fn as_async_state(
        &self,
        generics: &syn::Generics,
        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&syn::Type>,
    ) -> proc_macro2::TokenStream {
        use quote::quote;

        let mut parse_generics = generics.clone();
        let mut storage_generics = generics.clone();
        {
            parse_generics.params.push(syn::parse_quote!('a));
            parse_generics
                .params
                .push(syn::parse_quote!(O: ::#KRATE::reader::Offset + ::core::marker::Unpin));
            parse_generics.params.push(
                syn::parse_quote!(R: ::#KRATE::reader::PollReader<O> + ::core::marker::Unpin),
            );
        }
        {
            storage_generics.params.push(syn::parse_quote!(O));
        }

        let (parse_impl_generics, parse_ty_generics, parse_where_clause) =
            parse_generics.split_for_impl();
        let (_, storage_ty_generics, _) = storage_generics.split_for_impl();

        match self.group().unwrap() {
            GroupedConditionals::Enum { name, variants } => {
                let exhaustive = self.is_exhaustive();

                let parse_vs = variants.iter().map(|v| {
                    ConditionalVariant::format_variant_parse(
                        &v.variant_name,
                        exhaustive,
                        &parse_ty_generics,
                    )
                });

                let storage_vs = variants.iter().map(|v| {
                    ConditionalVariant::format_variant_storage(
                        &v.variant_name,
                        exhaustive,
                        &storage_ty_generics,
                    )
                });

                let variant_statemachines = variants.iter().map(|v| {
                    ConditionalVariant::format_variant_statemachine(
                        &v.variant_name,
                        &v.fields,
                        generics,
                        generic_collection,
                        &parse_generics,
                        &storage_generics,
                        payload_len,
                    )
                });

                let nested = variants.iter().flat_map(|v| {
                    v.fields.iter().filter_map(|field| {
                        if let AtomField::Conditional(cond) = field {
                            Some(cond.as_async_state(generics, generic_collection, payload_len))
                        } else {
                            None
                        }
                    })
                });

                let parse_name = quote::format_ident!("Async{}Parse", name);
                let storage_name = quote::format_ident!("Async{}Storage", name);

                let (none_parse_variant, none_storage_variant) =
                    ConditionalVariant::format_none_variants(exhaustive, &storage_generics);

                let variant_borrows = variants.iter().map(|v| {
                    ConditionalVariant::format_variant_borrow(&v.variant_name, exhaustive)
                });

                let none_variant_borrow = ConditionalVariant::format_none_borrow(exhaustive);

                quote! {
                    #(#nested)*
                    #(#variant_statemachines)*

                    pub enum #parse_name #parse_generics {
                        #(#parse_vs)*
                        #none_parse_variant
                    }

                    impl #parse_impl_generics ::#KRATE::reader::TakeReader<'a, R> for #parse_name #parse_ty_generics #parse_where_clause {
                        fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                            unreachable!("reader should not be taken from nested state")
                        }
                        fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                #(#variant_borrows)*
                                #none_variant_borrow
                            }
                        }
                    }

                    pub enum #storage_name #storage_generics {
                        #(#storage_vs)*
                        #none_storage_variant
                    }
                }
            }
            GroupedConditionals::Struct { name, fields } => {
                let exhaustive = false;
                let parse_variant =
                    ConditionalVariant::format_variant_parse(&name, exhaustive, &parse_ty_generics);

                let storage_variant = ConditionalVariant::format_variant_storage(
                    &name,
                    exhaustive,
                    &storage_ty_generics,
                );

                let variant_statemachine = ConditionalVariant::format_variant_statemachine(
                    &name,
                    &fields,
                    generics,
                    generic_collection,
                    &parse_generics,
                    &storage_generics,
                    payload_len,
                );

                let nested = fields.iter().filter_map(|field| {
                    if let AtomField::Conditional(cond) = field {
                        Some(cond.as_async_state(generics, generic_collection, payload_len))
                    } else {
                        None
                    }
                });

                let optional_name = quote::format_ident!("Optional{}", name);
                let parse_name = quote::format_ident!("Async{}Parse", optional_name);
                let storage_name = quote::format_ident!("Async{}Storage", optional_name);

                let (none_parse_variant, none_storage_variant) =
                    ConditionalVariant::format_none_variants(exhaustive, &storage_generics);

                let variant_borrow = ConditionalVariant::format_variant_borrow(&name, exhaustive);

                let none_variant_borrow = ConditionalVariant::format_none_borrow(exhaustive);

                quote! {
                    #(#nested)*
                    #variant_statemachine

                    pub enum #parse_name #parse_generics {
                        #parse_variant
                        #none_parse_variant
                    }

                    impl #parse_impl_generics ::#KRATE::reader::TakeReader<'a, R> for #parse_name #parse_ty_generics #parse_where_clause {
                        fn take_reader(self) -> (R, &'a ::#KRATE::parse_options::ParseOptions) {
                            unreachable!("reader should not be taken from nested state")
                        }
                        fn borrow_reader(&mut self) -> (&mut R, &'a ::#KRATE::parse_options::ParseOptions) {
                            match self {
                                #variant_borrow
                                #none_variant_borrow
                            }
                        }
                    }

                    pub enum #storage_name #storage_generics {
                        #storage_variant
                        #none_storage_variant
                    }
                }
            }
        }
    }

    pub fn as_async_field_collection(
        &self,
        mod_name: Option<&proc_macro2::TokenStream>,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        match self.group().unwrap() {
            GroupedConditionals::Struct {
                name: enum_name,
                fields,
            } => {
                let exhaustive = false;

                let field_name = &self.field_name;

                let variant_name = &enum_name;
                let enum_name = quote::format_ident!("Optional{}", enum_name);

                let variant = ConditionalVariant::format_field_collection(
                    variant_name,
                    &fields,
                    exhaustive,
                    mod_name,
                    &enum_name,
                    true,
                );
                let none_variant = ConditionalVariant::format_none_variant_collection(
                    exhaustive, mod_name, &enum_name,
                );

                quote! {
                    #field_name: match #field_name {
                        #variant
                        #none_variant
                    },
                }
            }
            GroupedConditionals::Enum {
                name: enum_name,
                variants,
            } => {
                let exhaustive = self.is_exhaustive();

                let field_name = &self.field_name;

                let vs = variants.iter().map(|v| {
                    ConditionalVariant::format_field_collection(
                        &v.variant_name,
                        &v.fields,
                        exhaustive,
                        mod_name,
                        &enum_name,
                        false,
                    )
                });

                let none_variant = ConditionalVariant::format_none_variant_collection(
                    exhaustive, mod_name, &enum_name,
                );

                quote! {
                    #field_name: match #field_name {
                        #(#vs)*
                        #none_variant
                    },
                }
            }
        }
    }
}
