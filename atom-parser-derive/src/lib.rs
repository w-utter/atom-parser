extern crate proc_macro;
use proc_macro::TokenStream;

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
    // just a normal field
    //
    // field: u8,
    Normal,
}

struct FourCC {

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
        } else {
            unreachable!("unknown attr");
        })
    }

    fn as_sync_parse(&self, field: &syn::Field) -> proc_macro2::TokenStream {
        let name = &field.ident;
        let ty = &field.ty;
        use quote::quote;
        match self {
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
            _ => todo!("3"),
        }
    }

    fn as_async_parse(&self, field: &syn::Field) -> proc_macro2::TokenStream {
        let name = &field.ident;
        let ty = &field.ty;
        use quote::quote;
        match self {
            Self::Reserved => {
                quote! {
                    //let _reserved = <#ty>::parse_async(reader, options).await?;
                    // TODO: check reserved fields
                }.into()
            },
            Self::DynamicArray {
                length_ty,
                zero_relative,
            } => {
                quote! {
                    let #name = DynamicArray::<#length_ty, #ty, #zero_relative>::parse_async(reader, options).await?;
                }
            }
            Self::TrailingArray => quote! {
                let #name = Trailing::<#ty>::parse_async(reader, options).await?;
            },
            Self::Payload => quote! {
                let #name = Payload::parse_async(reader, options).await?;
            },
            Self::Normal => {
                quote! {
                    let #name = <#ty>::parse_async(reader, options).await?;
                }.into()
            }
            _ => todo!("4"),
        }
    }

    fn as_collection(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        use quote::quote;
        Some(match self {
            Self::Reserved => return None,
            Self::Normal | Self::DynamicArray { .. } | Self::TrailingArray | Self::Payload => {
                quote! {
                    #name,
                }.into()
            }
            _ => todo!("5"),
        })
    }

    fn as_field_decl(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;

        use quote::quote;
        Some(match self {
            Self::Reserved => return None,
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
                    #name: DynamicArray<#length_ty, #ty, #zero_relative>,
                }
            }
            Self::TrailingArray => {
                quote! {
                    #name: Trailing<#ty>,
                }
            }
            Self::Payload => {
                quote! {
                    #name: Payload,
                }
            }
            _ => todo!("6"),
        })
    }
    
    fn as_helper_fn(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;

        use quote::quote;
        Some(match self {
            Self::Normal | Self::Reserved => return None,
            Self::TrailingArray => {
                quote! {
                    pub fn #name<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> TrailingIterator<'a, R, #ty> {
                        let trailing = &self.#name;
                        let max_offset = trailing.offset + trailing.size;

                        TrailingIterator {
                            reader: BacktrackReader::new(TrailingReader::new(reader, max_offset), trailing.offset),
                            opts,
                            _pd: core::marker::PhantomData,
                        }
                    }
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    pub fn #name<'a, R: Reader>(&'a self, reader: &'a mut R, opts: &'a ParseOptions) -> DynamicArrayIter<'a, R, #length_ty, #ty, #zero_relative> {
                        let arr = &self.#name;
                        DynamicArrayIter {
                            size: arr.size,
                            current: 0,
                            exhausted: false,
                            reader: BacktrackReader::new(reader, arr.offset),
                            opts,
                            _pd: core::marker::PhantomData,
                        }
                    }
                }
            }
            _ => todo!("a")
        })
    }
}

struct Flag {
    name: syn::Ident,
    val: syn::Expr,
    expected: bool,
}


impl syn::parse::Parse for Flag {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        if attrs.len() > 1 {
            panic!("more than 1 attr specified for flag")
        }
        let expected = attrs.into_iter().next().map(|attr| {
            if !attr.path().is_ident("expected") {
                panic!("unknown attr")
            }
            true
        }).unwrap_or(false);
        let _: syn::Token![const] = input.parse()?;
        let name = input.parse()?;
        let _: syn::Token![=] = input.parse()?;
        let val = input.parse()?;
        Ok(Self {
            name,
            val,
            expected
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

    fn flags_impl<'a>(name: &'a syn::Ident, flags: &'a [Flag], repr: &'a syn::Type) -> impl Iterator<Item = proc_macro2::TokenStream> + 'a {
        // TODO: this should cast to the Name type 
        // - s.t flags are stored as Flags rather than u32
        flags.iter().map(move |flag| {
            let flag_name = &flag.name;
            let flag_val = &flag.val;
            quote::quote!(const #flag_name: #name = Self::from_bits_(#flag_val as #repr);)
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
                        todo!()
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
    // the field that identifies the version number
    //
    // #[version]
    // version: u8,
    VersionIdentifier,
    // inline definitions of different versions
    //
    // #[version]
    // enum Version {
    //      #[version(0)]
    //      V1(/* verison specific */)
    // }
    Version,
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
        } else if path.is_ident("version") {
            todo!("8")
        } else if path.is_ident("children") {
            let _: syn::Token![enum] = input.parse()?;
            let _: syn::Ident = input.parse()?;
            let children = input.parse::<ChildList>()?.inner;
            Self::Children {
                size_ty: None,
                children,
            }
        } else {
            unreachable!("unknown atom attr")
        })
    }

    fn as_field_decl(&self, atom_mod: &syn::Ident) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(f, attr) => return attr.as_field_decl(f),
            Self::Children {
                size_ty,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        children: SizedChildren<#size_ty, #atom_mod::Child>
                    }
                } else {
                    quote! {
                        children: Children<#atom_mod::Child>
                    }
                }
            }
            Self::FullBox {
                name,
                ..
            } => {
                quote! {
                    atom_flags: #atom_mod::#name
                }
            }
            Self::Flags {
                field_name,
                name,
                ..
            } => {
                let field_name = field_name.clone().unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                quote! {
                    #field_name: #atom_mod::#name
                }
            }
            _ => todo!("9"),
        })
    }

    fn as_helper_fn(&self, atom_mod: &syn::Ident) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(f, attr) => return attr.as_helper_fn(f),
            Self::Children {
                size_ty,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    todo!()
                } else {
                    quote! {
                        pub fn children<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> ChildrenIter<'a, R, #atom_mod::Child> {
                            let children = &self.children;
                            let max_offset = children.offset + children.size;
                            ChildrenIter {
                                _pd: core::marker::PhantomData,
                                reader: BacktrackReader::new(TrailingReader::new(reader, max_offset), children.offset),
                                opts,
                            }
                        }
                    }
                }
            }
            Self::FullBox{ .. } | Self::Flags{ .. } => return None,
            _ => todo!("b"),
        })
    }

    fn as_async_parse(&self, atom_mod: &syn::Ident) -> proc_macro2::TokenStream {
        use quote::quote;
        match self {
            Self::Struct(field, attr) => attr.as_async_parse(field),
            Self::Children {
                size_ty,
                children,
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        let children = <SizedChildren<#size_ty, #atom_mod::Child>>::parse_async(reader, options).await?;
                    }
                } else {
                    quote! {
                        let children = <Children<#atom_mod::Child>>::parse_async(reader, options).await?;
                    }
                }
            }
            Self::FullBox {
                name,
                ..
            } => {
                quote! {
                    let (version_identifier, atom_flags) = {
                        let bits = u32::parse_async(reader, options).await?;
                        let [b0, b1, b2, b3] = u32::to_be_bytes(bits);
                        (b0, #atom_mod::#name::from_bytes([b1, b2, b3]))
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
                        let flags = <#parse_repr>::parse_async(reader, options).await?;
                        <#atom_mod::#name>::try_from_bits(flags, options)?
                    };
                }
            }
            _ => todo!("10"),
        }
    }

    fn as_sync_parse(&self, atom_mod: &syn::Ident) -> proc_macro2::TokenStream {
        use quote::quote;
        match self {
            Self::Struct(field, attr) => attr.as_sync_parse(field),
            Self::Children {
                size_ty,
                children,
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        let children = <SizedChildren<#size_ty, #atom_mod::Child>>::parse(reader, options)?;
                    }
                } else {
                    quote! {
                        let children = <Children<#atom_mod::Child>>::parse(reader, options)?;
                    }
                }
            }
            Self::FullBox {
                name,
                ..
            } => {
                quote! {
                    let (version_identifier, atom_flags) = {
                        let bits = u32::parse(reader, options)?;
                        let [b0, b1, b2, b3] = u32::to_ne_bytes(bits);
                        (b0, #atom_mod::#name::from_bytes([b1, b2, b3]))
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
                        <#atom_mod::#name>::try_from_bits(flags, options)?
                    };
                }
            }
            _ => todo!("11"),
        }
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
            _ => todo!("12"),
        })
    }

    fn as_inline_definition(&self) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match self {
            Self::Struct(_, _) => return None,
            Self::Children {
                size_ty,
                children,
            } => {
                let variants = children.iter().map(|child| &child.name).collect::<Vec<_>>();
                quote! {
                    pub enum Child {
                        #(
                            #variants(#variants)
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
                        async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                            /* TODO: async  impl (need TrailingReader support)
                            let atom = AtomHeader::parse_async(reader, options).await?;
                            let max_offset = reader.offset() + atom.size.size as usize;
                            let mut r = TrailingReader::new(&mut*reader, max_offset);
                            Ok(match atom.fcc {
                                #(
                                    <#variants as Atom>::FCC => {
                                        let atom = <#variants as Parse>::parse_async(&mut r, options)?;
                                        r.seek_remaining().await?;
                                        Child::#variants(atom)
                                    }
                                )*
                                missed => {
                                    r.seek_remaining().await?;
                                    Child::Unsupported(missed)
                                }
                            })
                            */
                            todo!()
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
                let flags = FlagList::flags_impl(name, flags, &repr);
                let flag_storage_elision = elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                let bitops_impl = FlagList::flags_bitops_impl(name);

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

                let flags = FlagList::flags_impl(name, flags, parse_repr);
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
            _ => todo!("13")
        })
    }
}

struct Child {
    name: syn::Ident,
    fcc: Option<FourCC>,
}

impl syn::parse::Parse for Child {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        // TODO: get override atom
        // eg
        // #[atom(abcd)]
        // ChilaA
        let name = input.parse()?;
        Ok(Self {
            name,
            fcc: None,
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

fn format_parse_impl(name: &syn::Ident, sync_parsing: &[proc_macro2::TokenStream], async_parsing: &[proc_macro2::TokenStream], field_collection: &[proc_macro2::TokenStream]) -> proc_macro2::TokenStream {
    quote::quote! {
        impl Parse for #name {
            fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                #(#sync_parsing)*
                Ok(Self {
                    #(#field_collection)*
                })
            }

            async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                #(#async_parsing)*
                Ok(Self {
                    #(#field_collection)*
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
    || path.is_ident("version")
    || path.is_ident("children")
}

fn is_struct_attr(path: &syn::Path) -> bool {
    path.is_ident("dynamic_array")
    || path.is_ident("reserved")
    || path.is_ident("pascal_string")
    || path.is_ident("null_terminated_string")
    || path.is_ident("trailing_array")
    || path.is_ident("payload")
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

enum Definition {
    Struct(syn::ItemStruct),
    Atom(AtomDefinition)
}

struct AtomFields {
    inner: Vec<AtomField>,
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

struct AtomDefinition {
    fcc: [u8; 4],
    attrs: Vec<syn::Attribute>,
    name: syn::Ident,
    fields: Vec<AtomField>,
}

impl AtomDefinition {
    fn group_inline_field_definitions(&self, atom_mod: &syn::Ident) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let inline_definitions = self.fields.iter().filter_map(|field| field.as_inline_definition());

        if inline_definitions.clone().count() == 0 {
            return None;
        }

        Some(quote! {
            mod #atom_mod {
                use super::*;
                #(#inline_definitions)*
            }
        })
    }

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

        let fcc = fcc.next().map(|(pos, _)| pos).map(|pos| attrs.swap_remove(pos)).map(|attr| {
            // TODO
            [0, 0, 0, 0]
        });

        Ok(match fcc {
            Some(fcc) => {
                let _: syn::Visibility = input.parse()?;
                let _: syn::Token![struct] = input.parse()?;
                let name = input.parse()?;
                let fields = input.parse::<AtomFields>()?.inner;

                Self::Atom (
                    AtomDefinition {
                        fcc,
                        attrs,
                        name,
                        fields,
                    }
                )
            }
            None => {
                Self::Struct(syn::ItemStruct {
                    attrs,
                    .. input.parse()?
                })
            }
        })
    }
}

struct DefinitionList {
    defs: Vec<Definition>,
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

#[proc_macro]
pub fn make_atom(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DefinitionList);

    use quote::quote;
    let defs = input.defs.iter().map(|def| {
        match def {
            Definition::Struct(item) => {
                // TODO: support for reserved and potentially other attributes ?
                quote!{
                    #item
                }
            }
            Definition::Atom(atom) => {
                let AtomDefinition {
                    fcc,
                    attrs,
                    name,
                    fields,
                } = &atom;

                let atom_mod = syn::Ident::new("test", proc_macro2::Span::call_site());
                let attrs = attrs.iter();
                // since some fields are omitted
                // (e.g reserved/padding)
                let atom_fields = fields.iter().filter_map(|field| field.as_field_decl(&atom_mod));

                let mod_specific = atom.group_inline_field_definitions(&atom_mod);

                let [fcc_0, fcc_1, fcc_2, fcc_3] = *fcc;

                let sync_parsing = fields.iter().map(|f| f.as_sync_parse(&atom_mod)).collect::<Vec<_>>();
                let async_parsing = fields.iter().map(|f| f.as_async_parse(&atom_mod)).collect::<Vec<_>>();
                let field_collection = fields.iter().filter_map(|f| f.as_collection()).collect::<Vec<_>>();

                let helper_fns = fields.iter().filter_map(|f| f.as_helper_fn(&atom_mod)).collect::<Vec<_>>();

                let parse_impl = format_parse_impl(name, &sync_parsing, &async_parsing, &field_collection);

                quote! {
                    #(#attrs)*
                    struct #name {
                        #(#atom_fields),*
                    }

                    impl #name {
                        #(#helper_fns)*
                    }

                    impl Atom for #name {
                        const FCC: FourCC = FourCC([#fcc_0, #fcc_1, #fcc_2, #fcc_3]);
                    }

                    #parse_impl

                    #mod_specific
                }
            }
        }
    });

    TokenStream::from(quote! {
        #(#defs)*
    })
}
