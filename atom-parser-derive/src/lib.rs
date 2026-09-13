extern crate proc_macro;
use proc_macro::TokenStream;

#[derive(Clone)]
enum StructFieldAttr {
    // definition of repeated fields `Entry` with length of type `Size`
    //
    // #[dynamic_array(zero_relative = false)]
    // some_field: [Entry; Size],
    // 
    DynamicArray {
        zero_relative: syn::LitBool,
        length_ty: syn::Type,
    },
    // declaration of reserved field
    //  
    // #[reserved]
    // reserved: [u8; 6],
    Reserved,
    // definition of a length prefixed string
    //
    // #[pascal_string(u32)]
    // some_field: String,
    PascalString {
        length_ty: syn::Type,
    },
    // definition of a null terminated string
    //
    // #[null_terminated_string]
    // some_field: String,
    NullTerminatedString,
    // declaration of a trailing array
    //  
    // #[trailing_array]
    // trailing: u32
    TrailingArray,
    // definition of trailing payload at the end of the atom 
    //
    // #[payload]
    // some_field: [u8],
    Payload,
    // the field that identifies the version number
    //
    // #[version]
    // version: u8,
    VersionIdentifier,
    // just a normal field
    //
    // field: u8,
    Normal,
}

#[derive(Clone)]
struct FourCC([u8; 4]);

impl FourCC {
    fn from_syn(attr: syn::Attribute) -> syn::parse::Result<Self> {
        let val = if let Ok(bstr) = attr.parse_args::<syn::LitByteStr>() {
            let bstr = bstr.value();
            if bstr.len() != 4 {
                panic!("fcc must be 4 characters")
            }
            let b = bstr;
            [b[0], b[1], b[2], b[3]]
        } else if let Ok(str) = attr.parse_args::<syn::LitStr>() {
            let str = str.value();
            if str.len() != 4 {
                panic!("fcc must be 4 characters")
            }
            let b = str.into_bytes();
            [b[0], b[1], b[2], b[3]]
        } else if let Ok(int) = attr.parse_args::<syn::LitInt>() {
            int.base10_parse::<u32>()?.to_be_bytes()
        } else {
            panic!("unknown repr for fourcc")
        };
        Ok(Self(val))
    }

    fn try_as_mod(&self) -> Option<&str> {
        let str = str::from_utf8(&self.0).ok()?;

        if !str.chars().all(|char| char.is_ascii_lowercase() || char.is_ascii_digit()) {
            return None;
        }
        Some(str)
    }

    fn as_ident(&self) -> syn::Ident {
        if let Some(name) = self.try_as_mod() {
            syn::Ident::new(name, proc_macro2::Span::call_site())
        } else {
            let num = u32::from_be_bytes(self.0);
            let name = format!("atom_{:#x}", num);
            syn::Ident::new(&name, proc_macro2::Span::call_site())
        }
    }

    fn as_fcc(&self) -> [u8; 4] {


        self.0
    }
}

impl StructFieldAttr {
    fn parse_from_syn(attr: &syn::Attribute) -> syn::parse::Result<Self> {
        let path = attr.path();
        Ok(if path.is_ident("dynamic_array") {
            let mut size_ty = None;
            let mut zero_relative = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("size_type") {
                    size_ty = Some(meta.value().expect("no ty for size_type").parse::<syn::Type>().expect("could not parse type of size_type"));
                    Ok(())
                } else if meta.path.is_ident("zero_relative") {
                    zero_relative = Some(meta.value().expect("no value for zero_relative").parse::<syn::LitBool>().expect("could not parse bool from zero_relative"));
                    Ok(())
                } else {
                    Err(meta.error("unsupported"))
                }
            })?;
            let length_ty = size_ty.unwrap();
            let zero_relative = zero_relative.unwrap();
            Self::DynamicArray {
                zero_relative,
                length_ty,
            }
        } else if path.is_ident("reserved") {
            Self::Reserved
        } else if path.is_ident("null_terminated_string") {
            Self::NullTerminatedString
        } else if path.is_ident("pascal_string") {
            let length_ty = attr.parse_args::<syn::Type>()?;
            Self::PascalString {
                length_ty,
            }
        } else if path.is_ident("trailing_array") {
            Self::TrailingArray
        } else if path.is_ident("payload") {
            Self::Payload
        } else if path.is_ident("version") {
            Self::VersionIdentifier
        } else {
            unreachable!("unknown attr");
        })
    }

    fn as_sync_parse(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        use quote::quote;
        Some(match self {
            Self::Reserved => {
                quote! {
                    let _reserved = <#ty>::parse(reader, options)?;
                    // TODO: check reserved fields
                }.into()
            },
            Self::DynamicArray {
                length_ty,
                zero_relative,
            } => {
                quote! {
                    let #name = DynamicArray::<#length_ty, #ty, #zero_relative>::parse(reader, options)?;
                }
            }
            Self::TrailingArray => quote! {
                let #name = Trailing::<#ty>::parse(reader, options)?;
            },
            Self::Payload => quote! {
                let #name = Payload::parse(reader, options)?;
            },
            Self::Normal => {
                quote! {
                    let #name = <#ty>::parse(reader, options)?;
                }.into()
            }
            Self::PascalString {
                length_ty
            } => {
                quote! {
                    let #name = PascalString::<#length_ty>::parse(reader, options)?;
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    let #name = NullTerminatedString::parse(reader, options)?;
                }
            }
            // builtin parsing in versioned impl
            Self::VersionIdentifier => return None,
        })
    }

    fn as_collection(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        use quote::quote;
        Some(match self {
            Self::Reserved | Self::VersionIdentifier => return None,
            Self::Normal | Self::DynamicArray { .. } | Self::TrailingArray | Self::Payload | Self::PascalString { .. } | Self::NullTerminatedString => {
                quote! {
                    #name,
                }.into()
            }
        })
    }

    fn as_field_decl(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;

        use quote::quote;
        Some(match self {
            Self::Reserved | Self::VersionIdentifier => return None,
            Self::Normal => {
                quote! {
                    #field
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    #name: DynamicArray<#length_ty, #ty, #zero_relative>
                }
            }
            Self::TrailingArray => {
                quote! {
                    #name: Trailing<#ty>
                }
            }
            Self::Payload => {
                quote! {
                    #name: Payload
                }
            }
            Self::PascalString {
                length_ty,
            } => {
                quote! {
                    #name: PascalString<#length_ty>
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    #name: NullTerminatedString
                }
            }
        })
    }
    
    fn as_helper_fn(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        let name = name.as_ref().unwrap();

        use quote::quote;
        Some(match self {
            Self::Normal | Self::Reserved | Self::VersionIdentifier => return None,
            Self::TrailingArray => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #name<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> TrailingIterator<'a, R, #ty> {
                        TrailingIterator::from_trailing(&self.#name, reader, opts)
                    }

                    pub fn #async_name<'a, R: Reader + SwapOffsets + PollReader + Unpin>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> AsyncTrailingIterator<'a, R, #ty> {
                        AsyncTrailingIterator::from_trailing(&self.#name, reader, opts)
                    }
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #name<'a, R: Reader>(&'a self, reader: &'a mut R, opts: &'a ParseOptions) -> DynamicArrayIter<'a, R, #length_ty, #ty, #zero_relative> {
                        DynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }

                    pub fn #async_name<'a, R: Reader + SwapOffsets + PollReader + Unpin>(&'a self, reader: &'a mut R, opts: &'a ParseOptions) -> AsyncDynamicArrayIter<'a, R, #length_ty, #ty, #zero_relative> {
                        AsyncDynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }
                }
            }
            // TODO
            Self::Payload => return None,
            Self::PascalString { .. } => return None,
            Self::NullTerminatedString => return None,
        })
    }
}

#[derive(Clone)]
struct Flag {
    name: syn::Ident,
    val: syn::Expr,
    expected: bool,
    version_num: HashSet<syn::LitInt>,
}

impl syn::parse::Parse for Flag {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        if attrs.len() > 2 {
            panic!("more than 2 attrs specified for flag")
        }

        let mut expected = None;
        let mut version_num = None;
        for attr in attrs.into_iter() {
            let path = attr.path();
            if path.is_ident("expected") {
                if expected.is_some() {
                    panic!("duplicate expected attrs")
                }
                expected = Some(true)
            } else if path.is_ident("version") {
                if version_num.is_some() {
                    panic!("duplicate version attrs")
                }
                version_num = Some(VersionList::parse_version_num_from_attr(attr)?)
            } else {
                panic!("unknown attr for field")
            }
        }
        let version_num = version_num.unwrap_or_default();
        let expected = expected.unwrap_or_default();

        let _: syn::Token![const] = input.parse()?;
        let name = input.parse()?;
        let _: syn::Token![=] = input.parse()?;
        let val = input.parse()?;
        Ok(Self {
            name,
            val,
            expected,
            version_num,
        })
    }
}

struct FlagList {
    flags: Vec<Flag>,
}

impl FlagList {
    fn can_elide_flag_storage(flags: &[Flag]) -> bool {
        if flags.iter().any(|flag| !flag.expected) {
            return false
        }
        true
    }

    fn flags_impl<'a>(name: &'a syn::Ident, flags: &'a [Flag], repr: &'a syn::Type, version: Option<&'a syn::LitInt>) -> impl Iterator<Item = proc_macro2::TokenStream> + 'a {
        flags.iter().filter_map(move |flag| {
            if let Some(v) = version {
                if !flag.version_num.is_empty() && !flag.version_num.contains(v) {
                    return None;
                }
            } else if !flag.version_num.is_empty() {
                panic!("version specified for flags but no version identifier")
            }

            let flag_name = &flag.name;
            let flag_val = &flag.val;
            Some(quote::quote!(const #flag_name: #name = Self::from_bits_(#flag_val as #repr);))
        })
    }

    fn flags_trait_impl(name: &syn::Ident, flags: &[Flag], parse_repr: &syn::Type) -> proc_macro2::TokenStream {
        // TODO: check flags if being annoying about it in parse options 

        let flags_check = Self::flags_parse_check(flags);

        quote::quote! {
            impl FlagsParse<#parse_repr> for #name {
                fn try_from_bits(bits: #parse_repr, options: &ParseOptions) -> Result<Self, ParseError> {
                    #(#flags_check)*

                    Ok(Self {
                        inner: bits,
                    })
                }
            }
        }
    }

    fn flags_bitops_impl(name: &syn::Ident) -> proc_macro2::TokenStream {
        quote::quote! {
            impl std::ops::BitAnd<#name> for #name {
                type Output = #name;
                fn bitand(self, rhs: #name) -> Self::Output {
                    use crate::Flags;
                    Self::from_bits(self.to_bits() & rhs.to_bits())
                }
            }

            impl std::ops::BitOr<#name> for #name {
                type Output = #name;
                fn bitor(self, rhs: #name) -> Self::Output {
                    use crate::Flags;
                    Self::from_bits(self.to_bits() | rhs.to_bits())
                }
            }

            impl std::ops::BitXor<#name> for #name {
                type Output = #name;
                fn bitxor(self, rhs: #name) -> Self::Output {
                    use crate::Flags;
                    Self::from_bits(self.to_bits() ^ rhs.to_bits())
                }
            }

            impl core::ops::Not for #name {
                type Output = #name;
                fn not(self) -> Self::Output {
                    use crate::Flags;
                    Self::from_bits(!self.to_bits())
                }
            }
        }
    }

    fn flags_parse_check(flags: &[Flag]) -> impl Iterator<Item = proc_macro2::TokenStream> {
        flags.iter().map(|flag| {
            quote::quote! {

            }
        })
    }

    fn debug_impl(flags: &[Flag], name: &syn::Ident) -> proc_macro2::TokenStream {
        use quote::quote;

        let flags_debug_nonelided = flags.iter().map(|flag| {
            // TODO
        });

        let flags_debug = flags.iter().map(|flag| {
            let flag_val = &flag.val;
            let name = &flag.name;
            quote! {
                if (val & #flag_val == #flag_val) {
                    if !first {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", stringify!(#name))?;
                    first = false;
                }
            }
        });

        quote! {
            impl core::fmt::Debug for #name {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    use crate::Flags;
                    let val = self.to_bits();
                    let mut first = true;
                    write!(f, "{}(", stringify!(#name))?;
                    #[cfg(feature = "store_unknown_fields")]
                    {
                        // TODO
                        //todo!()
                    }
                    #[cfg(not(feature = "store_unknown_fields"))]
                    {
                        #(#flags_debug)*
                    }
                    write!(f, ")")?;
                    Ok(())
                }
            }
        }
    }

    fn async_parse_impl(name: &syn::Ident, parse_repr: &syn::Type) -> proc_macro2::TokenStream {
        let parse_name = quote::format_ident!("Async{}Parse", name);
        quote::quote! {
            pub struct #parse_name<'a, R: PollReader + Unpin> {
                inner: <#parse_repr as AsyncParse>::Fut<'a, R>,
            }

            impl <'a, R: PollReader + Unpin> Future for #parse_name <'a, R> {
                type Output = Result<#name, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    let bits = core::task::ready!(core::pin::Pin::new(&mut self.inner).poll(cx))?;
                    let (_, opts) = self.borrow_reader();
                    let flags = #name::try_from_bits(bits, opts)?;
                    core::task::Poll::Ready(Ok(flags))
                }
            }

            impl AsyncParse for #name {
                type Fut<'a, R: PollReader + Unpin> = #parse_name<'a, R>;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    #parse_name {
                        inner: <#parse_repr>::create_fut(reader, options)
                    }
                }
            }

            impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for #parse_name<'a, R> {
                fn take_reader(self) -> (R, &'a ParseOptions) {
                    self.inner.take_reader()
                }
                fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                    self.inner.borrow_reader()
                }
            }
        }
    }
}

impl syn::parse::Parse for FlagList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        let content = content.parse_terminated(Flag::parse, syn::Token![;])?;
        let flags = content.into_iter().collect::<Vec<_>>();
        Ok(Self {
            flags,
        })
    }
}

#[derive(Clone)]
enum AtomField {
    // 1 byte of version followed by 3 bytes of flags
    // common at the start of some atoms
    //
    // #[full_box]
    // struct Flags {
    //      A = 1 << 0,
    //      B = 1 << 1,
    // }
    //
    // expands into an equivalent 
    //
    // #[version]
    // version: u8,
    // #[flags]
    // struct Flags: [u8; 3] {
    //      A = 1 << 0,
    //      B = 1 << 1,
    // }
    FullBox {
        name: syn::Ident,
        flags: Vec<Flag>,
    },

    // inline flag definitions inside the atom, with an optional field name
    //
    // #[flags(u32)]
    // <field_name: > struct Flags {
    //   const A = 1 << 0;
    //   const B = 1 << 1;
    // }
    // etc
    Flags {
        field_name: Option<syn::Ident>,
        name: syn::Ident,
        parse_repr: syn::Type,
        flags: Vec<Flag>
    },
    // inline definitions of different versions
    //
    // #[versions]
    // enum Version {
    //      #[version(0)]
    //      V1 {
    //         /* verison specific */
    //      },
    // }
    Version {
        versions: Vec<Version>,
    },
    // inline definitions of the atoms children
    //
    // #[children]
    // enum Children {
    //      Wide,
    //      Skip,
    // }
    Children {
        size_ty: Option<syn::Type>,
        children: Vec<Child>,
    },
    // attrs that can be shared with both structs and 
    Struct(syn::Field, StructFieldAttr),
}

fn format_async_statemachine(fields: &[AtomField], struct_name: &syn::Ident, mut generics: syn::Generics, atom_mod: Option<&syn::Ident>, generic_collection: Option<&proc_macro2::TokenStream>) -> proc_macro2::TokenStream {
    use quote::quote;
    let mod_name = atom_mod.map(|name| quote!(#name::));

    let parse_name = quote::format_ident!("Async{}Parse", struct_name);
    let typed_fields = fields.iter().enumerate().map(|(i, field)| {
        match field {
            AtomField::FullBox {name, ..} => {
                let ty = quote!(#mod_name #name);
                let name = syn::Ident::new("atom_flags", proc_macro2::Span::call_site());
                (name, ty, true)
            }
            AtomField::Flags {
                field_name,
                name,
                ..
            } => {
                let ty = quote!(#mod_name #name);
                let name = field_name.clone().unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                (name, ty, true)
            }
            AtomField::Version {..} => unreachable!(),
            AtomField::Children { size_ty, ..} => {
                let name = syn::Ident::new("children", proc_macro2::Span::call_site());

                let ty = if let Some(size_ty) = size_ty {
                    quote!(SizedChildren<#size_ty, #mod_name Child>)
                } else {
                    quote!(Children<#mod_name Child>)
                };
                (name, ty, true)
            }
            AtomField::Struct(field, attr) => {
                match attr {
                    StructFieldAttr::Normal => {
                        let name = field.ident.clone().unwrap();
                        let ty = &field.ty;
                        
                        (name, quote!(#ty), true)
                    }
                    StructFieldAttr::VersionIdentifier => unreachable!(),
                    StructFieldAttr::Payload => {
                        let name = field.ident.clone().unwrap();
                        (name, quote!(Payload), true)
                    }
                    StructFieldAttr::PascalString {
                        length_ty
                    } => {
                        let name = field.ident.clone().unwrap();
                        (name, quote!(PascalString<#length_ty>), true)
                    }
                    StructFieldAttr::NullTerminatedString => {
                        let name = field.ident.clone().unwrap();
                        (name, quote!(NullTerminatedString), true)
                    }
                    StructFieldAttr::Reserved => {
                        let name = quote::format_ident!("__reserved_{i}");
                        let ty = &field.ty;
                        (name, quote!(#ty), false)
                    }
                    StructFieldAttr::DynamicArray {
                        length_ty,
                        zero_relative,
                    } => {
                        let ty = &field.ty;
                        let name = field.ident.clone().unwrap();
                        (name, quote!(DynamicArray<#length_ty, #ty, #zero_relative>), true)
                    }
                    StructFieldAttr::TrailingArray => {
                        let ty = &field.ty;
                        let name = field.ident.clone().unwrap();
                        (name, quote!(Trailing<#ty>), true)
                    }
                }
            }
        }
    }).collect::<Vec<_>>();

    let mut state_machine_variants = vec![];
    let mut state_machine_variant_parsing = vec![];
    let mut borrow_reader_variants = vec![];

    for idx in 0..typed_fields.len() {
        let completed = &typed_fields[0..idx];
        let in_progress = &typed_fields[idx];
        let state = quote::format_ident!("S{}", idx);

        let variant_fields = completed.iter().filter_map(|(name, ty, display)| {
            if !display {
                return None;
            }

            Some(quote! {
                #name: #ty,
            })
        });


        let in_progress_fut = {
            let (name, ty, _) = in_progress;
            quote! {
                #name: <#ty as AsyncParse>::Fut<'a, R>,
            }
        };

        let variant = quote! {
            #state {
                #(#variant_fields)*
                #in_progress_fut
            }
        };

        let completed_names = completed.iter().filter_map(|(name, _, display)| {
            if !display {
                return None;
            }
            Some(name)
        }).collect::<Vec<_>>();
        let name = &in_progress.0;

        let borrow_reader = quote! {
            Self::#state {
                #name, ..
            } => #name.borrow_reader(),
        };

        let on_poll_ready = if idx == typed_fields.len() - 1 {
            let finished = typed_fields.iter().filter_map(|(name, _, display)| {
                if !display {
                    return None;
                }
                Some(name)
            });
            quote! {
                *self = Self::Done(reader, opts);
                return core::task::Poll::Ready(Ok(#struct_name {
                    #(#finished,)*
                    #generic_collection
                }));
            }
        } else {
            // get next state
            let (next_name, next_ty, _) = &typed_fields[idx + 1];
            let next_state = quote::format_ident!("S{}", idx + 1);

            let new_completed = typed_fields[..=idx].iter().filter_map(|(name, _, display)| {
                if !display {
                    return None;
                }
                Some(name)
            });

            quote! {
                let #next_name = <#next_ty>::create_fut(reader, opts);
                *self = Self::#next_state {
                    #(#new_completed,)*
                    #next_name
                };
            }
        };


        let match_fields = completed.iter().filter_map(|(name, _, display)| {
            if !display {
                return None;
            }

            Some(name)
        });

        let on_state = quote! {
            #parse_name::#state {
                #(#match_fields,)*
                mut #name,
            } => {
                match core::pin::Pin::new(&mut #name).poll(cx) {
                    core::task::Poll::Pending => {
                        *self = Self::#state {
                            #(#completed_names,)*
                            #name,
                        };
                        return core::task::Poll::Pending;
                    }
                    core::task::Poll::Ready(r) => {
                        let (reader, opts) = #name.take_reader();
                        let #name = r?;
                        #on_poll_ready
                    }
                }
            }
        };

        state_machine_variants.push(variant);
        state_machine_variant_parsing.push(on_state);
        borrow_reader_variants.push(borrow_reader);
    }

    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds.push(syn::parse_quote!(Unpin));
        }
    }

    let struct_generics = generics.clone();
    let (struct_impl_generics, struct_ty_generics, struct_where_clause) = struct_generics.split_for_impl();

    generics.params.push(syn::parse_quote!('a));
    generics.params.push(syn::parse_quote!(R: PollReader + Unpin));

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let (start, nop_variant, nop_poll, nop_borrow) = if let Some((name, ty, _)) = typed_fields.get(0) {
        (quote!{
            #name: <#ty as AsyncParse>::create_fut(reader, options)
        }, None, None, None)
    } else {
        (quote! {
            nop: (reader, options)
        }, Some(quote!{
            S0 { nop: (R, &'a ParseOptions) },
        }), Some(quote!{
            Self::S0 { nop: (reader, opts) } => {
                *self = Self::Done(reader, opts);
                return core::task::Poll::Ready(Ok(#struct_name { }))
            }
        }), Some(quote! {
            Self::S0 { nop: (reader, opts) } => (reader, opts),
        }))
    };

    quote! {
        pub enum #parse_name #generics {
            #(#state_machine_variants,)*
            #nop_variant
            Done(R, &'a ParseOptions),
            Empty,
        }

        impl #impl_generics Future for #parse_name #type_generics #where_clause {
            type Output = Result<#struct_name #struct_ty_generics, ParseError>;
            fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                loop {
                    let mut this = core::mem::replace(&mut *self, Self::Empty);
                    match this {
                        #(#state_machine_variant_parsing)*
                        #nop_poll
                        Self::Done(..) => panic!("future polled after completion"),
                        Self::Empty => unreachable!(),
                    }
                }
            }
        }

        impl #struct_impl_generics AsyncParse for #struct_name #struct_ty_generics #struct_where_clause {
            type Fut<'a, R: PollReader + Unpin> = #parse_name #type_generics ;
            fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                #parse_name ::S0 {
                    #start
                }
            }
        }

        impl #impl_generics TakeReader<'a, R> for #parse_name #type_generics #where_clause {
            impl_take_reader!{}
            fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                match self {
                    #(#borrow_reader_variants)*
                    #nop_borrow
                    Self::Done(reader, opts) => (reader, opts),
                    Self::Empty => unreachable!(),
                }
            }
        }
    }.into()
}

impl AtomField {
    fn parse_from_syn(input: syn::parse::ParseStream, atom_field_attr: &syn::Attribute, attrs: Vec<syn::Attribute>) -> syn::parse::Result<Self> {
        let _: syn::Visibility = input.parse()?;
        let path = atom_field_attr.path();
        Ok(if path.is_ident("full_box") {
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;
            let flags = input.parse::<FlagList>()?.flags;
            Self::FullBox {
                name,
                flags,
            }
        } else if path.is_ident("flags") {
            let field_name = input.parse()?;
            let _: syn::Token![:] = input.parse()?;
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;
            let parse_repr = atom_field_attr.parse_args::<syn::Type>()?;
            let flags = input.parse::<FlagList>()?.flags;

            Self::Flags {
                field_name,
                name,
                parse_repr,
                flags,
            }
        } else if path.is_ident("versions") {
            let _: syn::Token![enum] = input.parse()?;
            let _: syn::Ident = input.parse()?;

            let versions = input.parse::<VersionList>()?.inner;
            Self::Version {
                versions,
            }
        } else if path.is_ident("children") {
            let _: syn::Token![enum] = input.parse()?;
            let _: syn::Ident = input.parse()?;

            let size_ty = atom_field_attr.parse_args::<syn::Type>().ok();

            let children = input.parse::<ChildList>()?.inner;
            Self::Children {
                size_ty,
                children,
            }
        } else {
            unreachable!("unknown atom attr")
        })
    }

    fn as_field_decl(&self, atom_mod: Option<&syn::Ident>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = atom_mod.map(|name| quote!(#name::));

        Some(match self {
            Self::Struct(f, attr) => return attr.as_field_decl(f),
            Self::Children {
                size_ty,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub children: SizedChildren<#size_ty, #mod_name Child>
                    }
                } else {
                    quote! {
                        pub children: Children<#mod_name Child>
                    }
                }
            }
            Self::FullBox {
                name,
                ..
            } => {
                quote! {
                    pub atom_flags: #mod_name #name
                }
            }
            Self::Flags {
                field_name,
                name,
                ..
            } => {
                let field_name = field_name.clone().unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                quote! {
                    pub #field_name: #mod_name #name
                }
            }
            Self::Version { .. } => unreachable!(),
        })
    }

    fn as_helper_fn(&self, atom_mod: Option<&syn::Ident>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let mod_name = atom_mod.map(|name| quote!(#name::));
        Some(match self {
            Self::Struct(f, attr) => return attr.as_helper_fn(f),
            Self::Children {
                size_ty,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub fn children<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> DynamicArrayIter<'a, R, #size_ty, #mod_name Child, false> {
                            DynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }

                        pub fn children_async<'a, R: PollReader + Reader + Unpin>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> AsyncDynamicArrayIter<'a, R, #size_ty, #mod_name Child, false> {
                            AsyncDynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }
                    }
                } else {
                    quote! {
                        pub fn children<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> ChildrenIter<'a, R, #mod_name Child> {
                            ChildrenIter::from_children(&self.children, reader, opts)
                        }

                        pub fn children_async<'a, R: PollReader + Reader + Unpin>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> AsyncChildrenIter<'a, R, #mod_name Child> {
                            AsyncChildrenIter::from_children(&self.children, reader, opts)
                        }
                    }
                }
            }
            Self::FullBox{ .. } | Self::Flags{ .. } => return None,
            Self::Version{ .. } => unreachable!(),
        })
    }

    fn as_sync_parse(&self, atom_mod: &syn::Ident, version_mod: Option<&syn::Ident>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = match version_mod {
            Some(v) => quote!(#atom_mod::#v),
            None => quote!(#atom_mod)
        };

        Some(match self {
            Self::Struct(field, attr) => return attr.as_sync_parse(field),
            Self::Children {
                size_ty,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        let children = <SizedChildren<#size_ty, #mod_name::Child>>::parse(reader, options)?;
                    }
                } else {
                    quote! {
                        let children = <Children<#mod_name::Child>>::parse(reader, options)?;
                    }
                }
            }
            Self::FullBox {
                name,
                ..
            } => {
                quote! {
                    let atom_flags = {
                        let bytes = <[u8; 3]>::parse(reader, options)?;
                        #mod_name::#name::try_from_bits(bytes, options)?
                    };
                }
            }
            Self::Flags {
                name,
                parse_repr,
                field_name,
                ..
            } => {
                quote! {
                    let #field_name = {
                        use crate::FlagsParse;
                        let flags = <#parse_repr>::parse(reader, options)?;
                        <#mod_name::#name>::try_from_bits(flags, options)?
                    };
                }
            }
            Self::Version{..} => unreachable!(),
        })
    }

    fn as_collection(&self) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(field, attr) => return attr.as_collection(field),
            Self::Children {
                ..
            } => {
                quote! {
                    children,
                }
            }
            Self::FullBox {
                ..
            } => {
                quote! {
                    atom_flags,
                }
            }
            Self::Flags {
                field_name,
                ..
            } => {
                let field_name = field_name.clone().unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));

                quote! {
                    #field_name,
                }
            }
            Self::Version{..} => return None,
        })
    }

    fn as_inline_definition(&self, version: Option<&syn::LitInt>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(_, _) => return None,
            Self::Children {
                children,
                ..
            } => {
                let variants = children.iter().filter_map(|child| {
                    if let Some(v) = version {
                        if !child.version_num.is_empty() && !child.version_num.contains(v) {
                            return None
                        }
                    } else if !child.version_num.is_empty() {
                        panic!("child version but no version identifier");
                    }
                    Some(&child.name)
                }).collect::<Vec<_>>();

                quote! {
                    #[derive(Debug)]
                    pub enum Child {
                        #(
                            #variants(#variants),
                        )*
                        Unsupported(FourCC),
                    }

                    impl Parse for Child {
                        fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                            let atom = AtomHeader::parse(reader, options)?;
                            let max_offset = reader.offset() + atom.size.size as usize;
                            let mut r = TrailingReader::new(&mut*reader, max_offset);
                            Ok(match atom.fcc {
                                #(
                                    <#variants as Atom>::FCC => {
                                        let atom = <#variants as Parse>::parse(&mut r, options)?;
                                        r.seek_remaining()?;
                                        Child::#variants(atom)
                                    }
                                )*
                                missed => {
                                    r.seek_remaining()?;
                                    Child::Unsupported(missed)
                                }
                            })
                        }
                    }

                    pub enum AsyncChildParse<'a, R: PollReader + Unpin> {
                        AtomHeader(<AtomHeader as AsyncParse>::Fut<'a, R>),
                        #(
                            #variants(<#variants as AsyncParse>::Fut<'a, TrailingReader<R>>),
                        )*
                        Seek(Child, AsyncSeek<TrailingReader<R>>, &'a ParseOptions),
                        Done(TrailingReader<R>, &'a ParseOptions),
                        Empty,
                    }

                    impl AsyncParse for Child {
                        type Fut<'a, R: PollReader + Unpin> = AsyncChildParse<'a, R>;
                        fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                            AsyncChildParse::AtomHeader(AtomHeader::create_fut(reader, options))
                        }
                    }

                    impl <'a, R: PollReader + Unpin> Future for AsyncChildParse<'a, R> {
                        type Output = Result<Child, ParseError>;
                        fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                            loop {
                                let mut this = core::mem::replace(&mut*self, Self::Empty);
                                match this {
                                    Self::AtomHeader(mut header) => {
                                        match core::pin::Pin::new(&mut header).poll(cx) {
                                            core::task::Poll::Pending => {
                                                *self = Self::AtomHeader(header);
                                                return core::task::Poll::Pending;
                                            }
                                            core::task::Poll::Ready(res) => {
                                                let (reader, opts) = header.take_reader();

                                                let atom = res?;
                                                let max_offset = reader.offset() + atom.size.size as usize;
                                                let reader = TrailingReader::new(reader, max_offset);

                                                *self = match atom.fcc {
                                                    #(
                                                        <#variants as Atom>::FCC => {
                                                            Self::#variants(#variants::create_fut(reader, opts))
                                                        }
                                                    )*
                                                    u => {
                                                        let rest = reader.remaining_size();
                                                        Self::Seek(Child::Unsupported(u), async_impl::seek(reader, rest), opts)
                                                    }
                                                };
                                            }
                                        }
                                    }
                                    #(
                                        Self::#variants(mut fut) => {
                                            match core::pin::Pin::new(&mut fut).poll(cx) {
                                                core::task::Poll::Pending => {
                                                    *self = Self::#variants (fut);
                                                    return core::task::Poll::Pending;
                                                }
                                                core::task::Poll::Ready(res) => {
                                                    let child = Child::#variants(res?);
                                                    let (reader, opts) = fut.take_reader();
                                                    let rest = reader.remaining_size();
                                                    *self = Self::Seek(child, async_impl::seek(reader, rest), opts);
                                                }
                                            }
                                        }
                                    )*
                                    Self::Seek(child, mut fut, opts) => {
                                        match core::pin::Pin::new(&mut fut).poll(cx) {
                                            core::task::Poll::Pending => {
                                                *self = Self::Seek(child, fut, opts);
                                                return core::task::Poll::Pending;
                                            }
                                            core::task::Poll::Ready(res) => {
                                                let _ = res?;
                                                let reader = fut.reader;
                                                *self = Self::Done(reader, opts);
                                                return core::task::Poll::Ready(Ok(child))
                                            }
                                        }
                                    }
                                    Self::Done(..) => panic!("poll after completion"),
                                    Self::Empty => unreachable!(),
                                }
                            }
                        }
                    }

                    impl <'a, R: PollReader + Unpin> AsyncChildParse<'a, R> {
                        pub fn reader(&mut self) -> &mut TrailingReader<R> {
                            match self {
                                Self::Done(reader, _) => reader,
                                _ => unreachable!("should not be able to access mid future")
                            }
                        }
                    }

                    impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for AsyncChildParse<'a, R> {
                        fn take_reader(self) -> (R, &'a ParseOptions) {
                            match self {
                                Self::Done(reader, opts) => (reader.reader, opts),
                                _ => unreachable!("invalid state of AsyncChildParse")
                            }
                        }
                        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                            match self {
                                Self::AtomHeader(fut) => fut.borrow_reader(),
                                #(Self::#variants(fut) => {
                                    let (reader, opts) = fut.borrow_reader();
                                    (&mut reader.reader, opts)
                                })*
                                Self::Seek(_, seek, opts) => (&mut seek.reader.reader, opts),
                                Self::Done(reader, opts) => (&mut reader.reader, opts),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }
                }
            }
            Self::FullBox {
                name,
                flags,
            } => {
                let elide_flags = FlagList::can_elide_flag_storage(&flags);
                let flags_debug = FlagList::debug_impl(flags, name);

                let repr = syn::parse_quote!(u32);
                let flags_check = FlagList::flags_parse_check(flags);
                //let flags_impl = FlagList::flags_trait_impl(name, flags, &repr);
                let flags = FlagList::flags_impl(name, flags, &repr, version);
                let flag_storage_elision = elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                let bitops_impl = FlagList::flags_bitops_impl(name);
                let async_parse_repr = syn::parse_quote!([u8; 3]);
                let async_parse_impl = FlagList::async_parse_impl(name, &async_parse_repr);

                // TODO: bit ops

                quote! {
                    #[derive(Clone, Copy, PartialEq, Eq)]
                    pub struct #name {
                        //#flag_storage_elision
                        inner: [u8; 3],
                    }

                    impl #name {
                        #(#flags)*
                        pub fn from_bytes(bytes: [u8; 3]) -> Self {
                            Self {
                                //#flag_storage_elision
                                inner: bytes,
                            }
                        }

                        const fn from_bits_(bits: u32) -> Self {
                            let [b0, b1, b2, _] = bits.to_be_bytes();
                            Self {
                                inner: [b0, b1, b2]
                            }
                        }
                        const fn to_bits_(self) -> u32 {
                            let [b0, b1, b2] = self.inner;
                            u32::from_be_bytes([b0, b1, b2, 0])
                        }
                    }

                    impl FlagsParse<[u8; 3]> for #name {
                        fn try_from_bits(bits: [u8; 3], options: &ParseOptions) -> Result<Self, ParseError> {
                            {
                                let [b0, b1, b2] = bits;
                                let bits = u32::from_be_bytes([b0, b1, b2, 0]);
                                #(#flags_check)*
                            }

                            Ok(Self {
                                inner: bits,
                            })
                        }
                    }

                    impl crate::Flags<u32> for #name {
                        fn from_bits(bits: u32) -> Self {
                            Self::from_bits_(bits)
                        }

                        fn to_bits(self) -> u32 {
                            self.to_bits_()
                        }
                    }

                    #async_parse_impl

                    #bitops_impl
                    #flags_debug
                }
            }
            Self::Flags {
                name,
                parse_repr,
                flags,
                ..
            } => {
                let elide_flags = FlagList::can_elide_flag_storage(&flags);
                let flags_debug = FlagList::debug_impl(flags, name);
                let flags_trait_impl = FlagList::flags_trait_impl(name, flags, parse_repr);
                let bitops_impl = FlagList::flags_bitops_impl(name);
                let async_parse_impl = FlagList::async_parse_impl(name, parse_repr);

                let flags = FlagList::flags_impl(name, flags, parse_repr, version);
                let flag_storage_elision = elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                // TODO: bit ops

                quote! {
                    #[derive(Clone, Copy, PartialEq, Eq)]
                    pub struct #name {
                        //#flag_storage_elision
                        inner: #parse_repr,
                    }

                    impl #name {
                        #(#flags)*

                        const fn from_bits_(bits: #parse_repr) -> Self {
                            Self {
                                //#flag_storage_elision
                                inner: bits,
                            }
                        }

                        const fn to_bits_(self) -> #parse_repr {
                            self.inner
                        }
                    }

                    #flags_trait_impl
                    #flags_debug
                    #bitops_impl
                    #async_parse_impl

                    impl crate::Flags<#parse_repr> for #name {
                        fn from_bits(bits: #parse_repr) -> Self {
                            Self::from_bits_(bits)
                        }

                        fn to_bits(self) -> #parse_repr {
                            self.to_bits_()
                        }
                    }
                }
            }
            Self::Version{ .. } => return None,
        })
    }
}

#[derive(Clone)]
struct Child {
    name: syn::Ident,
    version_num: HashSet<syn::LitInt>,
}

impl syn::parse::Parse for Child {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;

        let mut version_num = None;
        for attr in attrs.into_iter() {
            let path = attr.path();
            if path.is_ident("version") {
                if version_num.is_some() {
                    panic!("duplicate version attrs")
                }
                version_num = Some(VersionList::parse_version_num_from_attr(attr)?);
            } else {
                panic!("unknown attr");
            }
        }
        let version_num = version_num.unwrap_or_default();

        let name = input.parse()?;
        Ok(Self {
            name,
            version_num,
        })
    }
}

struct ChildList {
    inner: Vec<Child>,
}

impl syn::parse::Parse for ChildList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let content = content.parse_terminated(Child::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();
        Ok(Self {
            inner,
        })
    }
}

#[derive(Clone)]
struct Version {
    version_num: HashSet<syn::LitInt>,
    version_name: syn::Ident,
    fields: Vec<AtomField>,
}

impl syn::parse::Parse for Version {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        if attrs.len() != 1 {
            panic!("incorrect # of attrs (only need #[version(0)])")
        }

        let version_attr = attrs
            .into_iter()
            .next()
            .unwrap();

        if !version_attr.path().is_ident("version") {
            panic!("unknown attr")
        }

        let version_num = VersionList::parse_version_num_from_attr(version_attr)?;
        let version_name = input.parse()?;
        let content;
        syn::braced!(content in input);
        let content = content.parse_terminated(AtomField::parse, syn::Token![,])?;
        let fields = content.into_iter().collect::<Vec<_>>();

        Ok(Self {
            version_num,
            version_name,
            fields,
        })
    }
}

struct VersionList {
    inner: Vec<Version>,
}

impl VersionList {
    fn parse_version_num_from_attr(attr: syn::Attribute) -> syn::parse::Result<HashSet<syn::LitInt>> {
        Ok(attr.parse_args_with(syn::punctuated::Punctuated::<syn::LitInt, syn::Token![,]>::parse_terminated)?.into_iter().collect::<HashSet<_>>())
    }
}

impl syn::parse::Parse for VersionList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);

        let content = content.parse_terminated(Version::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();
        Ok(Self {
            inner,
        })

    }
}

fn format_parse_impl(name: &syn::Ident, sync_parsing: &[proc_macro2::TokenStream], field_collection: &[proc_macro2::TokenStream], generics: &syn::Generics, generic_collection: Option<&proc_macro2::TokenStream>) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote::quote! {
        impl #impl_generics Parse for #name #ty_generics #where_clause {
            fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                #(#sync_parsing)*
                Ok(Self {
                    #(#field_collection)*
                    #generic_collection
                })
            }
        }
    }
}

fn parse_normal_field(input: syn::parse::ParseStream, attrs: Vec<syn::Attribute>) -> syn::parse::Result<syn::Field> {
    Ok(syn::Field {
        attrs,
        ..syn::Field::parse_named(input)?
    })
}

fn is_atom_attr(path: &syn::Path) -> bool {
    path.is_ident("full_box")
    || path.is_ident("flags")
    || path.is_ident("versions")
    || path.is_ident("children")
}

fn is_struct_attr(path: &syn::Path) -> bool {
    path.is_ident("dynamic_array")
    || path.is_ident("reserved")
    || path.is_ident("pascal_string")
    || path.is_ident("null_terminated_string")
    || path.is_ident("trailing_array")
    || path.is_ident("payload")
    || path.is_ident("version")
}

impl syn::parse::Parse for AtomField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;

        enum AttrKind {
            Atom,
            Struct,
        }

        let mut atom_attrs = attrs.iter().enumerate().filter_map(|(i, attr)| {
            let path = attr.path();

            Some(if is_atom_attr(path) {
                (i, AttrKind::Atom)
            } else if is_struct_attr(path) {
                (i, AttrKind::Struct)
            } else {
                return None
            })
        });

        if atom_attrs.clone().count() > 1 {
            panic!("more than 1 attribute specifying fields")
        }

        let atom_attr = atom_attrs.next().map(|(idx, kind)| (attrs.swap_remove(idx), kind));

        Ok(match atom_attr {
            Some((attr, AttrKind::Atom)) => {
                Self::parse_from_syn(input, &attr, attrs)?
            }
            Some((attr, AttrKind::Struct)) => {
                Self::Struct(parse_normal_field(input, attrs)?, StructFieldAttr::parse_from_syn(&attr)?)
            }
            None => {
                Self::Struct(parse_normal_field(input, attrs)?, StructFieldAttr::Normal)
            }
        })
    }
}

enum DefinitionKind {
    Struct(StructDefinition),
    Atom(AtomDefinition),
    Enum(EnumDefinition),
}

struct AtomFields {
    inner: Vec<AtomField>,
}

impl AtomFields {
    fn group_inline_definitions(fields: &[AtomField], mod_name: &syn::Ident, version: Option<&syn::LitInt>) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let inline_definitions = fields.iter().filter_map(|field| field.as_inline_definition(version));

        if inline_definitions.clone().count() == 0 {
            return None;
        }

        Some(quote! {
            mod #mod_name {
                use super::*;
                #(#inline_definitions)*
            }
        })
    }
}

impl syn::parse::Parse for AtomFields {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        let _ = syn::braced!(content in input);

        let content = content.parse_terminated(AtomField::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();

        Ok(Self {
            inner,
        })
    }
}

use std::collections::{HashMap, HashSet};

fn try_expand_into_versioned_fields(fields: Vec<AtomField>, version: Option<syn::LitInt>) -> syn::Result<Result<Versioned, Vec<AtomField>>> {
    let versions = fields.iter().filter(|field| matches!(field, AtomField::Version { .. }));
    let mut version_identifier = fields.iter().enumerate().filter(|(_, field)| matches!(field, AtomField::FullBox { .. } | AtomField::Struct(_, StructFieldAttr::VersionIdentifier)));

    if version_identifier.clone().count() > 1 {
        panic!("more than 1 version identifier")
    }

    match (version_identifier.next(), &version, versions.count()) {
        (None, Some(_), _) => panic!("verion attr but no version identifier"),
        (None, _, vs) if vs > 0 => panic!("verions present but no version identifier"),
        (Some(_), None, v) if v == 0 => panic!("version identifier but no verisons"),
        (_, Some(_), v) if v > 0 => panic!("version attr and versions present"),
        (Some((i, _)), _, _) if i > 0 => panic!("version identifier needs to be declared as the first field"),
        (None, _, _) => return Ok(Err(fields)),
        (_, Some(version_num), _) => {
            let mut map = HashMap::new();

            let mut version_repr = None;
            let mut out_fields = vec![];

            for field in &fields {
                match field {
                    AtomField::Struct(field, StructFieldAttr::VersionIdentifier) => {
                        version_repr = Some(field.ty.clone());
                    }
                    AtomField::Version { .. } => unreachable!(),
                    field => {
                        if matches!(field, AtomField::FullBox { .. }) {
                            let repr = syn::parse_quote!(u8);
                            version_repr = Some(repr);
                        }
                        out_fields.push(field.clone());
                    }
                }
            }

            map.insert(version_num.clone(), out_fields);
            Ok(Ok(Versioned {
                versions: map,
                version_repr: version_repr.unwrap()
            }))
        }
        _ => {
            let mut map = HashMap::new();

            let mut version_independent = vec![];
            let mut version_repr = None;

            for field in &fields {
                match field {
                    AtomField::Struct(field, StructFieldAttr::VersionIdentifier) => {
                        version_repr = Some(field.ty.clone());
                    }
                    AtomField::Version {
                        versions,
                    } => {
                        for version in versions {
                            for v in &version.version_num {
                                if !map.contains_key(v) {
                                    map.insert(v.clone(), version_independent.clone());
                                }
                                let version_specific = map.get_mut(v).unwrap();
                                version_specific.extend(version.fields.clone());
                            }
                        }
                    }
                    field => {
                        if matches!(field, AtomField::FullBox { .. }) {
                            let repr = syn::parse_quote!(u8);
                            version_repr = Some(repr);
                        }
                        for (_, version_specific) in &mut map {
                            version_specific.push(field.clone());
                        }
                        version_independent.push(field.clone());
                    }
                }
            }

            Ok(Ok(Versioned {
                versions: map,
                version_repr: version_repr.unwrap(),
            }))
        }
    }
}


struct AtomDefinition {
    fcc: FourCC,
    attrs: Vec<syn::Attribute>,
    name: syn::Ident,
    fields: Vec<AtomField>,
}

impl AtomDefinition {
    fn parse_from_syn(input: syn::parse::ParseStream, fcc: FourCC, attrs: Vec<syn::Attribute>, name: syn::Ident) -> syn::Result<Self> {
        let fields = input.parse::<AtomFields>()?.inner;
        Ok(Self {
            fcc,
            attrs,
            name,
            fields,
        })
    }
}

struct StructDefinition {
    attrs: Vec<syn::Attribute>,
    name: syn::Ident,
    generics: syn::Generics,
    fields: Vec<AtomField>,
}

impl StructDefinition {
    fn parse_from_syn(input: syn::parse::ParseStream, attrs: Vec<syn::Attribute>, name: syn::Ident) -> syn::Result<Self> {
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

    fn module_name(&self) -> syn::Ident {
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

impl AtomDefinition {
    fn verify_definition(&self) -> syn::parse::Result<()> {
        // TODO: 
        // make sure that 
        // - 1 or less trailing field
        // - #[version] identifier must have a matching #[version] implementation
        Ok(())
    }
}

impl syn::parse::Parse for Definition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;

        let mut fcc = attrs.iter().enumerate().filter(|(_, attr)| attr.path().is_ident("atom"));

        if fcc.clone().count() > 1 {
            panic!("more than 1 atom attr specified")
        }

        let fcc = fcc.next().map(|(pos, _)| pos).map(|pos| attrs.swap_remove(pos)).map(|attr| FourCC::from_syn(attr)).transpose()?;

        let mut version = attrs.iter().enumerate().filter(|(_, attr)| attr.path().is_ident("version"));

        if version.clone().count() > 1 {
            panic!("more than 1 version");
        }

        let version = version.next().map(|(pos, _)| pos).map(|pos| attrs.swap_remove(pos)).map(|attr| attr.parse_args::<syn::LitInt>()).transpose()?;

        let _: syn::Visibility = input.parse()?;

        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Token![struct]) {
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;

            let kind = match fcc {
                Some(fcc) => {
                    DefinitionKind::Atom (AtomDefinition::parse_from_syn(input, fcc, attrs, name)?)
                }
                None => {
                    DefinitionKind::Struct(StructDefinition::parse_from_syn(input, attrs, name)?)
                }
            };

            Ok(Self {
                version,
                kind,
            })
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

struct Definition {
    version: Option<syn::LitInt>,
    kind: DefinitionKind,
}

struct DefinitionList {
    defs: Vec<Definition>,
}

struct Versioned {
    version_repr: syn::Type,
    versions: HashMap<syn::LitInt, Vec<AtomField>>,
}

impl Versioned {
    fn version_mod_from_lit(v: &syn::LitInt) -> syn::Ident {
        quote::format_ident!("v{v}")
    }

    fn versioned_struct_from_lit(name: &syn::Ident, v: &syn::LitInt) -> syn::Ident {
        quote::format_ident!("{name}V{v}")
    }

    fn versioned_enum_variant_from_lit(v: &syn::LitInt) -> syn::Ident {
        quote::format_ident!("V{v}")
    }

    fn format_enum_name_from_versioned_struct(name: &syn::Ident) -> syn::Ident {
        quote::format_ident!("{name}Versions")
    }

    fn format_versioned_struct(&self, name: &syn::Ident, attrs: &[syn::Attribute], generics: &syn::Generics) -> proc_macro2::TokenStream {
        use quote::quote;

        let mut generics = generics.clone();
        for param in &mut generics.params {
            if let syn::GenericParam::Type(ty) = param {
                ty.bounds.push(syn::parse_quote!(Unpin));
            }
        }

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let enum_name = Self::format_enum_name_from_versioned_struct(name);
        let enum_variants = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);
            quote!(#variant(#version_mod::#versioned_name #ty_generics),)
        });

        let sync_parsing = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);

            quote!(#v => Self::#variant(#version_mod::#versioned_name::parse(reader, options)?),)
        });

        let async_sm_variants = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);
            quote!(#variant(<#version_mod::#versioned_name #ty_generics as AsyncParse>::Fut<'a, R>))
        });

        let async_parse_enum_name = quote::format_ident!("Async{}Parse", enum_name);

        let async_parse_impl = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            quote!(Self::#variant(mut fut) => {
                match core::pin::Pin::new(&mut fut).poll(cx) {
                    core::task::Poll::Pending => {
                        *self = Self::#variant(fut);
                        return core::task::Poll::Pending;
                    }
                    core::task::Poll::Ready(res) => {
                        let (reader, opts) = fut.take_reader();
                        *self = Self::Done(reader, opts);
                        return core::task::Poll::Ready(Ok(#enum_name::#variant(res?)));
                    }
                }
            })
        });

        let borrow_reader_impl = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            quote!(Self::#variant(fut) => fut.borrow_reader(),)
        });

        let async_create_fut = self.versions.iter().map(|(v, _)| {
            let variant = Self::versioned_enum_variant_from_lit(v);
            let version_mod = Self::version_mod_from_lit(v);
            let versioned_name = Self::versioned_struct_from_lit(&name, v);

            quote! {
                #v => #async_parse_enum_name::#variant(<#version_mod::#versioned_name #ty_generics>::create_fut(reader, opts)),
            }
        });

        let version_repr = &self.version_repr;

        let mut async_parse_generics = generics.clone();
        async_parse_generics.params.push(syn::parse_quote!('a));
        async_parse_generics.params.push(syn::parse_quote!(R: PollReader + Unpin));

        let (async_impl_generics, async_ty_generics, async_where_clause) = async_parse_generics.split_for_impl();

        quote! {
            #(#attrs)*
            #[derive(Debug)]
            pub enum #enum_name #generics {
                #(#enum_variants)*
                Unknown(#version_repr),
            }

            use super::*;
            impl #impl_generics Parse for #enum_name #ty_generics #where_clause {
                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                    let version_ident = <#version_repr>::parse(reader, options)?;
                    Ok(match version_ident {
                        #(#sync_parsing)*
                        u => Self::Unknown(u),
                    })
                }
            }

            impl #impl_generics #enum_name #ty_generics #where_clause {
                pub fn create_fut_from_version<'a, R: PollReader + Unpin>(version: #version_repr, reader: R, opts: &'a ParseOptions) -> #async_parse_enum_name #async_ty_generics {
                    match version {
                        #(#async_create_fut)*
                        u => {
                            let remaining = reader.remaining_size();
                            let seek = async_impl::seek(reader, remaining);
                            #async_parse_enum_name::Unknown(u, seek, opts)
                        }
                    }
                }
            }

            pub enum #async_parse_enum_name #async_parse_generics {
                #(#async_sm_variants,)*
                Unknown(#version_repr, AsyncSeek<R>, &'a ParseOptions),
                Done(R, &'a ParseOptions),
                Empty,
            }

            impl #impl_generics AsyncParse for #enum_name #ty_generics #where_clause {
                type Fut<'a, R: PollReader + Unpin> = #async_parse_enum_name #async_ty_generics;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    unreachable!("can only be called when version is available")
                }
            }

            impl #async_impl_generics Future for #async_parse_enum_name #async_ty_generics #async_where_clause {
                type Output = Result<#enum_name #ty_generics, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    loop {
                        let mut this = core::mem::replace(&mut *self, Self::Empty);
                        match this {
                            #(#async_parse_impl)*
                            Self::Unknown(version, mut seek, opts) => {
                                match core::pin::Pin::new(&mut seek).poll(cx) {
                                    core::task::Poll::Pending => {
                                        *self = Self::Unknown(version, seek, opts);
                                        return core::task::Poll::Pending;
                                    }
                                    core::task::Poll::Ready(res) => {
                                        let _ = res?;
                                        let reader = seek.reader;
                                        *self = Self::Done(reader, opts);
                                        return core::task::Poll::Ready(Ok(#enum_name::Unknown(version)));
                                    }
                                }
                            }
                            Self::Done(..) => panic!("poll after completion"),
                            Self::Empty => unreachable!(),
                        }
                    }
                }
            }

            impl #async_impl_generics TakeReader<'a, R> for #async_parse_enum_name #async_ty_generics #async_where_clause {
                impl_take_reader!{}
                fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                    match self {
                        #(#borrow_reader_impl)*
                        Self::Unknown(_, seek, opts) => (&mut seek.reader, opts),
                        Self::Done(reader, opts) => (reader, opts),
                        Self::Empty => unreachable!(),
                    }
                }
            }
        }
    }

    fn format_async_parse(&self, name: &syn::Ident, generics: &syn::Generics, atom_mod: &syn::Ident) -> proc_macro2::TokenStream {
        use quote::quote;
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let mut async_parse_generics = generics.clone();
        async_parse_generics.params.push(syn::parse_quote!('a));
        async_parse_generics.params.push(syn::parse_quote!(R: PollReader + Unpin));
        let (async_impl_generics, async_ty_generics, async_where_clause) = async_parse_generics.split_for_impl();

        let enum_name = Self::format_enum_name_from_versioned_struct(name);
        let async_parse_enum_name = quote::format_ident!("Asnyc{}Parse", name);
        let parse_repr = &self.version_repr;

        quote! {
            pub enum #async_parse_enum_name #async_parse_generics {
                Version(<#parse_repr as AsyncParse>::Fut<'a, R>),
                VersionSpecific(<#atom_mod::#enum_name #ty_generics as AsyncParse>::Fut<'a, R>),
                Done(R, &'a ParseOptions),
                Empty,
            }

            impl #async_impl_generics Future for #async_parse_enum_name #async_ty_generics #async_where_clause {
                type Output = Result<#name #ty_generics, ParseError>;
                fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                    loop {
                        let mut this = core::mem::replace(&mut*self, Self::Empty);
                        match this {
                            Self::Version(mut fut) => {
                                match core::pin::Pin::new(&mut fut).poll(cx) {
                                    core::task::Poll::Pending => {
                                        *self = Self::Version(fut);
                                        return core::task::Poll::Pending;
                                    }
                                    core::task::Poll::Ready(v) => {
                                        let (reader, opts) = fut.take_reader();
                                        *self = Self::VersionSpecific(<#atom_mod::#enum_name>::create_fut_from_version(v?, reader, opts));
                                    }
                                }
                            }
                            Self::VersionSpecific(mut fut) => {
                                match core::pin::Pin::new(&mut fut).poll(cx) {
                                    core::task::Poll::Pending => {
                                        *self = Self::VersionSpecific(fut);
                                        return core::task::Poll::Pending;
                                    }
                                    core::task::Poll::Ready(version) => {
                                        let (reader, opts) = fut.take_reader();
                                        *self = Self::Done(reader, opts);
                                        return core::task::Poll::Ready(version.map(|version| {
                                            #name {
                                                version,
                                            }
                                        }));
                                    }
                                }
                            }
                            Self::Done(..) => panic!("poll after completion"),
                            Self::Empty => unreachable!(),
                        }
                    }
                }
            }

            impl #async_impl_generics TakeReader<'a, R> for #async_parse_enum_name #async_ty_generics #async_where_clause {
                impl_take_reader!{}
                fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                    match self {
                        Self::Version(v) => v.borrow_reader(),
                        Self::VersionSpecific(s) => s.borrow_reader(),
                        Self::Done(r, opts) => (r, opts),
                        Self::Empty => unreachable!(),
                    }
                }
            }

            impl #impl_generics AsyncParse for #name #ty_generics #where_clause {
                type Fut<'a, R: PollReader + Unpin> = #async_parse_enum_name #async_ty_generics;
                fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                    #async_parse_enum_name::Version(<#parse_repr>::create_fut(reader, options))
                }
            }
        }
    }
}

impl syn::parse::Parse for DefinitionList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut defs = vec![];
        while !input.is_empty() {
            defs.push(input.parse()?);
        }
        Ok(DefinitionList {
            defs,
        })
    }
}

struct EnumDefinition {
    attrs: Vec<syn::Attribute>,
    name: syn::Ident,
    variants: Vec<EnumVariant>,
    repr: syn::Type,
}

impl EnumDefinition {
    fn parse_from_syn(input: syn::parse::ParseStream, mut attrs: Vec<syn::Attribute>) -> syn::Result<Self> {
        let mut repr = attrs.iter().enumerate().filter(|(_, attr)| attr.path().is_ident("enum_repr"));

        if repr.clone().count() > 1 {
            panic!("more than 1 repr attr specified");
        }

        let repr = repr.next().map(|(pos, _)| pos).map(|pos| attrs.swap_remove(pos)).map(|attr| attr.parse_args::<syn::Type>()).transpose()?.expect("no size attr specified for enum");

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
        Ok(Self {
            variants,
        })
    }
}

struct EnumVariant {
    name: syn::Ident,
    val: syn::Expr,
}

impl syn::parse::Parse for EnumVariant {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _: syn::Token![=] = input.parse()?;
        let val = input.parse()?;

        Ok(Self {
            name,
            val,
        })
    }
}

#[proc_macro]
pub fn make_atom(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DefinitionList);

    use quote::quote;

    let defs = input.defs.into_iter().map(|def| {
        match def.kind {
            DefinitionKind::Struct(mut item) => {
                for param in &mut item.generics.params {
                    if let syn::GenericParam::Type(type_param) = param {
                        type_param.bounds.push(syn::parse_quote!(Parse));
                        type_param.bounds.push(syn::parse_quote!(Unpin));
                    }
                }

                let atom_mod = item.module_name();
                let StructDefinition {
                    attrs,
                    name,
                    fields,
                    generics,
                } = item;

                match try_expand_into_versioned_fields(fields, def.version) {
                    Ok(Ok(versioned)) => {
                        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                        let version_specific = versioned.versions.iter().map(|(v, fields)| {
                            let version_mod = Versioned::version_mod_from_lit(v);

                            let version_specific = fields.iter().filter_map(|field| field.as_inline_definition(Some(v)));
                            let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, Some(&version_mod))).collect::<Vec<_>>();
                            let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();
                            let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(None)).collect::<Vec<_>>();

                            let name = Versioned::versioned_struct_from_lit(&name, v);

                            // so that if one version has a generic and another doesnt, all versions
                            // are bounded by the same generics
                            let (generic_storage, generic_collection) = if generics.params.is_empty() {
                                (None, None)
                            } else {
                                let lts = generics.params.iter().filter_map(|param| {
                                    if let syn::GenericParam::Lifetime(lt) = param {
                                        Some(lt.lifetime.clone())
                                    } else {
                                        None
                                    }
                                });
                                let rest = generics.params.iter().filter_map(|param| {
                                    use syn::GenericParam;
                                    Some(match param {
                                        GenericParam::Type(ty) => ty.ident.clone(),
                                        GenericParam::Const(c) => c.ident.clone(),
                                        _ => return None,
                                    })
                                });

                                let items = quote!(#(#lts,)*#(#rest,)*);
                                (
                                    Some(quote!(_pd: core::marker::PhantomData< (( #items )) >)),
                                    Some(quote!(_pd: core::marker::PhantomData,))
                                )
                            };

                            let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &generics, generic_collection.as_ref());
                            let async_parse_impl = format_async_statemachine(fields, &name, generics.clone(), None, generic_collection.as_ref());
                            let fields = fields.iter().filter_map(|field| field.as_field_decl(None));

                            quote! {
                                pub mod #version_mod {
                                    use super::*;
                                    #(#attrs)*
                                    #[derive(Debug)]
                                    pub struct #name #generics {
                                        #(#fields,)*
                                        #generic_storage
                                    }

                                    impl #impl_generics #name #ty_generics #where_clause{
                                        #(#helper_fns)*
                                    }

                                    #parse_impl
                                    #async_parse_impl
                                    #(#version_specific)*
                                }
                            }
                        });

                        let mut generics = generics.clone();

                        for param in &mut generics.params {
                            if let syn::GenericParam::Type(ty) = param {
                                ty.bounds.push(syn::parse_quote!(Unpin));
                            }
                        }

                        let versions = versioned.format_versioned_struct(&name, &attrs, &generics);
                        let enum_name = Versioned::format_enum_name_from_versioned_struct(&name);

                        let async_parse_impl = versioned.format_async_parse(&name, &generics, &atom_mod);

                        quote! {
                            pub mod #atom_mod {
                                #versions
                                #(#version_specific)*
                            }

                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name #generics {
                                pub version: #atom_mod::#enum_name #ty_generics,
                            }

                            impl #impl_generics Parse for #name #ty_generics #where_clause {
                                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                                    let version = #atom_mod::#enum_name::parse(reader, options)?;
                                    Ok(Self {
                                        version,
                                    })
                                }
                            }
                            #async_parse_impl
                        }
                    }
                    Ok(Err(fields)) => {
                        let attrs = attrs.iter();
                        // since some fields are omitted
                        // (e.g reserved/padding)
                        let atom_fields = fields.iter().filter_map(|field| field.as_field_decl(Some(&atom_mod)));

                        let mod_specific = AtomFields::group_inline_definitions(&fields, &atom_mod, None);

                        let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, None)).collect::<Vec<_>>();
                        let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();

                        let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(Some(&atom_mod))).collect::<Vec<_>>();

                        let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &generics, None);
                        let async_parse_impl = format_async_statemachine(&fields, &name, generics.clone(), Some(&atom_mod), None);

                        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                        quote! {
                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name #generics {
                                #(#atom_fields),*
                            }

                            impl #impl_generics #name #ty_generics #where_clause {
                                #(#helper_fns)*
                            }

                            #parse_impl

                            #async_parse_impl

                            #mod_specific
                        }
                    }
                    Err(e) => {
                        todo!()
                    }
                }
            }
            DefinitionKind::Atom(atom) => {
                let AtomDefinition {
                    fcc,
                    attrs,
                    name,
                    fields,
                } = atom;

                let atom_mod = fcc.as_ident();
                let [fcc_0, fcc_1, fcc_2, fcc_3] = fcc.as_fcc();
                match try_expand_into_versioned_fields(fields, def.version) {
                    Ok(Ok(versioned)) => {
                        let version_specific = versioned.versions.iter().map(|(v, fields)| {
                            let version_mod = Versioned::version_mod_from_lit(v);

                            let version_specific = fields.iter().filter_map(|field| field.as_inline_definition(Some(v)));
                            let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, Some(&version_mod))).collect::<Vec<_>>();
                            let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();
                            let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(None)).collect::<Vec<_>>();

                            let name = Versioned::versioned_struct_from_lit(&name, v);

                            let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &Default::default(), None);
                            let async_parse_impl = format_async_statemachine(fields, &name, Default::default(), None, None);
                            let fields = fields.iter().filter_map(|field| field.as_field_decl(None));

                            quote! {
                                pub mod #version_mod {
                                    use super::*;
                                    #(#attrs)*
                                    #[derive(Debug)]
                                    pub struct #name {
                                        #(#fields),*
                                    }

                                    impl #name {
                                        #(#helper_fns)*
                                    }

                                    #parse_impl
                                    #async_parse_impl
                                    #(#version_specific)*
                                }
                            }
                        });

                        let versions = versioned.format_versioned_struct(&name, &attrs, &Default::default());
                        let enum_name = Versioned::format_enum_name_from_versioned_struct(&name);

                        let async_parse_impl = versioned.format_async_parse(&name, &Default::default(), &atom_mod);

                        quote! {
                            pub mod #atom_mod {
                                #versions
                                #(#version_specific)*
                            }

                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name {
                                pub version: #atom_mod::#enum_name,
                            }

                            impl Parse for #name {
                                fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                                    let version = #atom_mod::#enum_name::parse(reader, options)?;
                                    Ok(Self {
                                        version,
                                    })
                                }
                            }

                            #async_parse_impl

                            impl Atom for #name {
                                const FCC: FourCC = FourCC([#fcc_0, #fcc_1, #fcc_2, #fcc_3]);
                            }
                        }
                    }
                    Ok(Err(fields)) => {
                        let attrs = attrs.iter();
                        // since some fields are omitted
                        // (e.g reserved/padding)
                        let atom_fields = fields.iter().filter_map(|field| field.as_field_decl(Some(&atom_mod)));

                        let mod_specific = AtomFields::group_inline_definitions(&fields, &atom_mod, None);

                        let sync_parsing = fields.iter().filter_map(|f| f.as_sync_parse(&atom_mod, None)).collect::<Vec<_>>();
                        let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();

                        let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(Some(&atom_mod))).collect::<Vec<_>>();

                        let parse_impl = format_parse_impl(&name, &sync_parsing, &field_collection, &Default::default(), None);
                        let async_parse_impl = format_async_statemachine(&fields, &name, Default::default(), Some(&atom_mod), None);

                        quote! {
                            #(#attrs)*
                            #[derive(Debug)]
                            pub struct #name {
                                #(#atom_fields),*
                            }

                            impl #name {
                                #(#helper_fns)*
                            }

                            impl Atom for #name {
                                const FCC: FourCC = FourCC([#fcc_0, #fcc_1, #fcc_2, #fcc_3]);
                            }

                            #parse_impl
                            #async_parse_impl

                            #mod_specific
                        }
                    }
                    Err(e) => todo!()
                }
            }
            DefinitionKind::Enum(e) => {
                let EnumDefinition {
                    attrs,
                    name,
                    variants,
                    repr,
                } = e;

                let enum_variants = variants.iter().map(|v| &v.name);

                let try_from_impl = variants.iter().map(|v| {
                    let val = &v.val;
                    let name = &v.name;
                    quote!{
                        #val => Self::#name
                    }
                });

                let async_parse_name = quote::format_ident!("Async{}Parse", name);

                quote! {
                    #(#attrs)*
                    #[derive(Debug)]
                    pub enum #name {
                        #(#enum_variants,)*
                        Unknown(#repr),
                    }

                    impl #name {
                        pub fn try_from_bits(bits: #repr) -> Self {
                            match bits {
                                #(#try_from_impl,)*
                                u => Self::Unknown(u),
                            }
                        }
                    }

                    impl Parse for #name {
                        fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                            let repr = <#repr>::parse(reader, options)?;
                            Ok(Self::try_from_bits(repr))
                        }
                    }

                    pub enum #async_parse_name<'a, R: PollReader + Unpin> {
                        Repr(<#repr as AsyncParse>::Fut<'a, R>),
                        Done(R, &'a ParseOptions),
                        Empty,
                    }

                    impl AsyncParse for #name {
                        type Fut<'a, R: PollReader + Unpin> = #async_parse_name<'a, R>;
                        fn create_fut<'a, R: PollReader + Unpin>(reader: R, options: &'a ParseOptions) -> Self::Fut<'a, R> {
                           #async_parse_name::Repr(<#repr>::create_fut(reader, options))
                        }
                    }

                    impl <'a, R: PollReader + Unpin> Future for #async_parse_name<'a, R> {
                        type Output = Result<#name, ParseError>;
                        fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                            let mut this = core::mem::replace(&mut *self, Self::Empty);
                            match this {
                                Self::Repr(mut fut) => {
                                    match core::pin::Pin::new(&mut fut).poll(cx) {
                                        core::task::Poll::Pending => {
                                            *self = Self::Repr(fut);
                                            return core::task::Poll::Pending;
                                        }
                                        core::task::Poll::Ready(res) => {
                                            let repr = res?;
                                            let (reader, opts) = fut.take_reader();
                                            *self = Self::Done(reader, opts);
                                            return core::task::Poll::Ready(Ok(#name::try_from_bits(repr)));
                                        }
                                    }
                                }
                                Self::Done(..) => panic!("poll after completion"),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }

                    impl <'a, R: PollReader + Unpin> TakeReader<'a, R> for #async_parse_name<'a, R> {
                        impl_take_reader!{}
                        fn borrow_reader(&mut self) -> (&mut R, &'a ParseOptions) {
                            match self {
                                Self::Repr(r) => r.borrow_reader(),
                                Self::Done(reader, opts) => (reader, opts),
                                Self::Empty => unreachable!(),
                            }
                        }
                    }
                }
            }
        }
    });

    TokenStream::from(quote! {
        #(#defs)*
    })
}
