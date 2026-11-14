use crate::{
    Child, ChildList, Conditionals, Endianess, Flag, FlagList, GroupedConditionals, KRATE, Version,
    VersionList,
};

#[derive(Clone)]
pub struct AtomField {
    pub endianess: Option<Endianess>,
    pub kind: AtomFieldKind,
}

#[derive(Clone)]
pub enum AtomFieldKind {
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
        flags: Vec<Flag>,
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
    //
    // OR
    //
    // inline definitions of a tagged item variant
    // #[tagged]
    // enum <enum_name> {
    //      SomeTagVariant,
    //      SomeOtherTagVariant,
    // }
    Children {
        size_ty: Option<syn::Type>,
        children: Vec<Child>,
        kind: ChildrenKind,
    },
    Conditional(Conditionals),

    // attrs that can be shared with both structs and
    Struct(syn::Field, StructFieldAttr),
}

fn parse_normal_field(
    input: syn::parse::ParseStream,
    attrs: Vec<syn::Attribute>,
) -> syn::parse::Result<syn::Field> {
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
        || path.is_ident("tagged")
        || path.is_ident("conditional")
}

fn is_struct_attr(path: &syn::Path) -> bool {
    path.is_ident("dynamic_array")
        || path.is_ident("reserved")
        || path.is_ident("expected")
        || path.is_ident("pascal_string")
        || path.is_ident("null_terminated_string")
        || path.is_ident("null_terminated_array")
        || path.is_ident("trailing_array")
        || path.is_ident("payload")
        || path.is_ident("payload_length")
        || path.is_ident("tag")
        || path.is_ident("padded")
        || path.is_ident("condition")
        || path.is_ident("reuse_condition")
        || path.is_ident("version")
}

impl syn::parse::Parse for AtomField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        let endianess = crate::definition::singular_attr(&mut attrs, "endianess", |attr| {
            attr.parse_args_with(Endianess::parse_from_syn)
        })?;

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
                return None;
            })
        });

        if atom_attrs.clone().count() > 1 {
            panic!("more than 1 attribute specifying fields")
        }

        let atom_attr = atom_attrs
            .next()
            .map(|(idx, kind)| (attrs.swap_remove(idx), kind));

        let kind = match atom_attr {
            Some((attr, AttrKind::Atom)) => AtomFieldKind::parse_from_syn(input, &attr)?,
            Some((attr, AttrKind::Struct)) => AtomFieldKind::Struct(
                parse_normal_field(input, attrs)?,
                StructFieldAttr::parse_from_syn(&attr)?,
            ),
            None => {
                AtomFieldKind::Struct(parse_normal_field(input, attrs)?, StructFieldAttr::Normal)
            }
        };

        Ok(Self { kind, endianess })
    }
}

pub struct AtomFields {
    pub inner: Vec<AtomField>,
}

impl AtomFields {
    pub fn group_inline_definitions(
        fields: &[AtomField],
        mod_name: &syn::Ident,
        version: Option<&syn::LitInt>,
        attrs: &[syn::Attribute],
        generics: &syn::Generics,

        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&PayloadLength>,
        tag: Option<&Tag>,
        definition_endianess: Option<&Endianess>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let payload_len = AtomFields::nested_payload_len(payload_len, fields);
        let tag = AtomFields::nested_tag(tag, fields);
        let inline_definitions = fields.iter().filter_map(|field| {
            field.as_inline_definition(
                version,
                attrs,
                generics,
                generic_collection,
                payload_len.as_ref(),
                tag.as_ref(),
                definition_endianess,
            )
        });

        if inline_definitions.clone().count() == 0 {
            return None;
        }

        Some(quote! {
            pub mod #mod_name {
                use super::*;
                #(#inline_definitions)*
            }
        })
    }

    pub fn tag(fields: &[AtomField]) -> Option<Tag> {
        let mut tags = fields.iter().filter_map(|field| match &field.kind {
            AtomFieldKind::Struct(field, StructFieldAttr::Tag) => Some(&field.ty),
            _ => None,
        });

        if tags.clone().count() > 1 {
            panic!("more than one tag specified");
        }

        let mut tagged = fields.iter().filter(|field| {
            matches!(
                &field.kind,
                AtomFieldKind::Children {
                    kind: ChildrenKind::Tagged { .. },
                    ..
                }
            )
        });

        if tagged.clone().count() > 1 {
            panic!("more than one tagged specified")
        }
        match (tags.next(), tagged.next()) {
            (Some(ty), Some(_)) => Some(Tag { ty: ty.clone() }),
            (None, None) => None,
            (tag, tagged) => panic!(
                "tag was {} and tagged was {}",
                tag.is_some(),
                tagged.is_some()
            ),
        }
    }

    pub fn nested_tag(existing: Option<&Tag>, fields: &[AtomField]) -> Option<Tag> {
        match (existing, Self::tag(fields)) {
            (Some(_), Some(new)) => Some(new),
            (Some(existing), None) => Some(existing.clone()),
            (None, Some(new)) => Some(new),
            (None, None) => None,
        }
    }

    pub fn payload_len(fields: &[AtomField]) -> Option<PayloadLength> {
        let mut tys = fields.iter().filter_map(|field| match &field.kind {
            AtomFieldKind::Struct(field, StructFieldAttr::PayloadLength { offset, padding }) => {
                Some((&field.ty, offset, padding))
            }
            _ => None,
        });

        if tys.clone().count() > 1 {
            panic!("more than one payload length specified");
        }

        let ty = tys.next();
        let mut trailing = fields.iter().enumerate().filter(|(_, field)| {
            matches!(
                &field.kind,
                AtomFieldKind::Children { .. }
                    | AtomFieldKind::Struct(
                        _,
                        StructFieldAttr::Payload { .. }
                            | StructFieldAttr::TrailingArray { .. }
                            | StructFieldAttr::Padded { .. }
                    )
            )
        });
        let trailing_count = trailing.clone().count();

        if trailing_count > 1 {
            panic!("more than 1 trailing collection")
        }

        let ty = match (ty, trailing.next()) {
            (
                Some(_),
                Some((
                    _,
                    AtomField {
                        kind:
                            AtomFieldKind::Children {
                                kind: ChildrenKind::Atom,
                                ..
                            },
                        ..
                    },
                )),
            ) => panic!(
                "length specified with children is unsupported (it is gotten from the atom size)"
            ),
            (_, Some((i, _))) if i != fields.len() - 1 => {
                panic!("trailing field is not the last field declared")
            }
            (Some(_), None) => panic!("trailing length specified but no trailing field"),
            (Some((ty, offset, padding)), Some(_)) => Some(PayloadLength {
                ty: ty.clone(),
                offset: offset.clone(),
                padding: padding.clone(),
            }),
            (None, _) => None,
        };

        ty
    }

    pub fn nested_payload_len(
        existing: Option<&PayloadLength>,
        fields: &[AtomField],
    ) -> Option<PayloadLength> {
        match (existing, Self::payload_len(fields)) {
            (Some(_), Some(new)) => Some(new),
            (Some(existing), None) => Some(existing.clone()),
            (None, Some(new)) => Some(new),
            (None, None) => None,
        }
    }
}

impl syn::parse::Parse for AtomFields {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        let _ = syn::braced!(content in input);

        let content = content.parse_terminated(AtomField::parse, syn::Token![,])?;
        let inner = content.into_iter().collect::<Vec<_>>();

        Ok(Self { inner })
    }
}

impl AtomFieldKind {
    pub fn parse_from_syn(
        input: syn::parse::ParseStream,
        atom_field_attr: &syn::Attribute,
    ) -> syn::parse::Result<Self> {
        let _: syn::Visibility = input.parse()?;
        let path = atom_field_attr.path();
        Ok(if path.is_ident("full_box") {
            let _: syn::Token![struct] = input.parse()?;
            let name = input.parse()?;
            let flags = input.parse::<FlagList>()?.flags;
            Self::FullBox { name, flags }
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
            Self::Version { versions }
        } else if path.is_ident("children") {
            let _: syn::Token![enum] = input.parse()?;
            let _: syn::Ident = input.parse()?;

            let size_ty = atom_field_attr.parse_args::<syn::Type>().ok();

            let children = input.parse::<ChildList>()?.inner;
            Self::Children {
                size_ty,
                children,
                kind: ChildrenKind::Atom,
            }
        } else if path.is_ident("tagged") {
            let field_name = input.parse()?;
            let _: syn::Token![:] = input.parse()?;
            let _: syn::Token![enum] = input.parse()?;
            let tagged_name: syn::Ident = input.parse()?;

            let f = || {
                use syn::Meta;
                match &atom_field_attr.meta {
                    Meta::Path(_) => return Ok((false, None)),
                    Meta::List(list) => {
                        let parser = |input: syn::parse::ParseStream| {
                            let fork = input.fork();
                            if let Ok(path) = fork.parse::<syn::Path>() {
                                if path.is_ident("skip_seek") {
                                    let _: syn::Path = input.parse()?;
                                    return Ok((true, None));
                                }
                            }
                            Ok((false, Some(LenFromExpr::parse_from_syn(input)?)))
                        };
                        use syn::parse::Parser;
                        parser.parse2(list.tokens.clone())
                    }
                    _ => panic!("unsupported kv meta"),
                }
            };
            let (skip_seek, len_from_expr) = f()?;

            let children = input.parse::<ChildList>()?.inner;
            Self::Children {
                size_ty: None,
                children,
                kind: ChildrenKind::Tagged {
                    field_name,
                    tagged_name,
                    len_from_expr,
                    skip_seek,
                },
            }
        } else if path.is_ident("conditional") {
            use syn::Meta;
            let name = match &atom_field_attr.meta {
                Meta::Path(_) => None,
                Meta::List(list) => {
                    let parser = |input: syn::parse::ParseStream| Ok(Some(input.parse()?));
                    use syn::parse::Parser;
                    parser.parse2(list.tokens.clone())?
                }
                Meta::NameValue(_) => panic!("name-value pair for conditional is not supported"),
            };

            let cond = Conditionals::parse_from_syn(name, input)?;
            Self::Conditional(cond)
        } else {
            unreachable!("unknown atom attr")
        })
    }
}

impl AtomField {
    pub fn as_field_decl(
        &self,
        atom_mod: Option<&syn::Ident>,
        payload_len: Option<&PayloadLength>,
        generics: &syn::Generics,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = atom_mod.map(|name| quote!(#name::));

        Some(match &self.kind {
            AtomFieldKind::Struct(f, attr) => return attr.as_field_decl(f, payload_len),
            AtomFieldKind::Children { size_ty, kind, .. } => match (kind, size_ty) {
                (ChildrenKind::Atom, Some(size_ty)) => {
                    quote! {
                        pub children: ::#KRATE::children::SizedChildren<__Offset, #size_ty, #mod_name Child>
                    }
                }
                (ChildrenKind::Atom, None) => {
                    quote! {
                        pub children: ::#KRATE::children::Children<__Offset, #mod_name Child>
                    }
                }
                (
                    ChildrenKind::Tagged {
                        field_name,
                        tagged_name,
                        ..
                    },
                    _,
                ) => {
                    quote! {
                        pub #field_name: <#mod_name #tagged_name as ::#KRATE::parse::Parsed>::Output<__Offset>
                    }
                }
            },
            AtomFieldKind::Conditional(cond) => {
                let exhaustive = cond.is_exhaustive();
                let name = &cond.field_name;
                let ty = match cond.group().unwrap() {
                    GroupedConditionals::Struct { name, .. } => name,
                    GroupedConditionals::Enum { name, .. } => name,
                };

                let mut generics = generics.clone();
                generics.params.push(syn::parse_quote!(__Offset = usize));

                let (_, ty_generics, _) = generics.split_for_impl();

                if !exhaustive {
                    quote! {
                        pub #name: Option<#mod_name #ty #ty_generics>
                    }
                } else {
                    quote! {
                        pub #name: #mod_name #ty #ty_generics
                    }
                }
            }
            AtomFieldKind::FullBox { name, .. } => {
                quote! {
                    pub atom_flags: #mod_name #name
                }
            }
            AtomFieldKind::Flags {
                field_name, name, ..
            } => {
                let field_name = field_name
                    .clone()
                    .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                quote! {
                    pub #field_name: #mod_name #name
                }
            }
            AtomFieldKind::Version { .. } => unreachable!(),
        })
    }

    pub fn as_variant_decl(
        &self,
        payload_len: Option<&PayloadLength>,
        generics: &syn::Generics,
    ) -> Option<proc_macro2::TokenStream> {
        match &self.kind {
            AtomFieldKind::Struct(field, StructFieldAttr::ReuseCondition) => {
                let this = AtomField {
                    kind: AtomFieldKind::Struct(field.clone(), StructFieldAttr::Normal),
                    endianess: self.endianess.clone(),
                };
                this.as_field_decl(None, payload_len, generics)
            }
            _ => self.as_field_decl(None, payload_len, generics),
        }
    }

    pub fn as_sync_helper_fn(
        &self,
        atom_mod: Option<&syn::Ident>,
        definition_endianess: Option<&Endianess>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let mod_name = atom_mod.map(|name| quote!(#name::));
        let format_options = Endianess::format_options(
            self.endianess.as_ref(),
            Some(Endianess::format_options_create_fut(
                definition_endianess,
                None,
                false,
            )),
            false,
        );
        let options_ty = Endianess::format_options_ty(
            self.endianess.as_ref(),
            Some(Endianess::format_options_ty(definition_endianess, None)),
        );
        Some(match &self.kind {
            AtomFieldKind::Struct(f, attr) => {
                return attr.as_sync_helper_fn(f, &format_options, &options_ty);
            }
            AtomFieldKind::Children {
                kind: ChildrenKind::Tagged { .. },
                ..
            } => return None,
            AtomFieldKind::Children {
                size_ty,
                kind: ChildrenKind::Atom,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub fn children<__Options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader:  __Reader, opts: __Options) -> ::#KRATE::array::DynamicArrayIter<#options_ty, __Offset, __Reader, #size_ty, #mod_name Child, false> {
                            #format_options
                            ::#KRATE::array::DynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }
                    }
                } else {
                    quote! {
                        pub fn children<__Options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::children::ChildrenIter<#options_ty, __Offset, __Reader, #mod_name Child> {
                            #format_options
                            ::#KRATE::children::ChildrenIter::from_children(&self.children, reader, opts)
                        }
                    }
                }
            }
            AtomFieldKind::FullBox { .. }
            | AtomFieldKind::Flags { .. }
            | AtomFieldKind::Conditional(..) => return None,
            AtomFieldKind::Version { .. } => unreachable!(),
        })
    }

    pub fn as_async_helper_fn(
        &self,
        atom_mod: Option<&syn::Ident>,
        definition_endianess: Option<&Endianess>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        let mod_name = atom_mod.map(|name| quote!(#name::));
        let format_options = Endianess::format_options(
            self.endianess.as_ref(),
            Some(Endianess::format_options_create_fut(
                definition_endianess,
                None,
                false,
            )),
            false,
        );
        let options_ty = Endianess::format_options_ty(
            self.endianess.as_ref(),
            Some(Endianess::format_options_ty(definition_endianess, None)),
        );
        Some(match &self.kind {
            AtomFieldKind::Struct(f, attr) => {
                return attr.as_async_helper_fn(f, &format_options, &options_ty);
            }
            AtomFieldKind::Children {
                kind: ChildrenKind::Tagged { .. },
                ..
            } => return None,
            AtomFieldKind::Children {
                size_ty,
                kind: ChildrenKind::Atom,
                ..
            } => {
                if let Some(size_ty) = size_ty {
                    quote! {
                        pub fn children_async<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::array::AsyncDynamicArrayIter<#options_ty, __Offset, __Reader, #size_ty, #mod_name Child, false> {
                            #format_options
                            ::#KRATE::array::AsyncDynamicArrayIter::from_sized_children(&self.children, reader, opts)
                        }
                    }
                } else {
                    quote! {
                        pub fn children_async<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::children::AsyncChildrenIter<#options_ty, __Offset, __Reader, #mod_name Child> {
                            #format_options
                            ::#KRATE::children::AsyncChildrenIter::from_children(&self.children, reader, opts)
                        }
                    }
                }
            }
            AtomFieldKind::FullBox { .. }
            | AtomFieldKind::Flags { .. }
            | AtomFieldKind::Conditional(..) => return None,
            AtomFieldKind::Version { .. } => unreachable!(),
        })
    }

    pub fn as_sync_parse(
        &self,
        version: Option<&syn::LitInt>,
        atom_mod: &syn::Ident,
        version_mod: Option<&syn::Ident>,
        payload_len: Option<&PayloadLength>,
        tag: Option<&Tag>,
        definition_endianess: Option<&Endianess>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;

        let mod_name = match version_mod {
            Some(v) => quote!(#atom_mod::#v),
            None => quote!(#atom_mod),
        };

        let format_options = Endianess::format_options(
            self.endianess.as_ref(),
            Some(Endianess::format_options_create_fut(
                definition_endianess,
                None,
                true,
            )),
            true,
        );

        Some(match &self.kind {
            AtomFieldKind::Struct(field, attr) => {
                return attr.as_sync_parse(field, payload_len, &format_options);
            }
            AtomFieldKind::Children {
                size_ty,
                kind,
                children,
            } => match (kind, size_ty) {
                (ChildrenKind::Atom, Some(size_ty)) => {
                    quote! {
                        let children = {
                            #format_options
                            <::#KRATE::children::SizedChildren<__Offset, #size_ty, #mod_name::Child>>::parse(reader, &opts)?
                        };
                    }
                }
                (ChildrenKind::Atom, None) => {
                    quote! {
                        let children = {
                            #format_options
                            <::#KRATE::children::Children<__Offset, #mod_name::Child>>::parse(reader, &opts)?
                        };
                    }
                }
                (
                    ChildrenKind::Tagged {
                        field_name,
                        tagged_name,
                        len_from_expr,
                        skip_seek,
                    },
                    _,
                ) => {
                    let reader = reader_from_lengths(len_from_expr.as_ref(), payload_len);

                    let tag_ty = &tag.expect("no tag for tagged").ty;

                    let tag_bindings =
                        ChildList::bind_overridden_tag_consts(&children, version, tag_ty);

                    let sync_parse = ChildList::sync_parse(
                        &children,
                        version,
                        tag_ty,
                        Some(&mod_name),
                        tagged_name,
                        false,
                        *skip_seek,
                    );

                    quote! {
                        let #field_name = {
                            #format_options
                            #(#tag_bindings)*

                            let mut reader = #reader;
                            match _tag {
                                #(#sync_parse)*
                                missed => {
                                    let extent = ::#KRATE::reader::Reader::extent(&reader);
                                    ::#KRATE::reader::Reader::seek_remaining(&mut reader)?;
                                    #mod_name :: #tagged_name::Unsupported(::#KRATE::children::UnknownChild::new(missed, extent))
                                }
                            }
                        };
                    }
                }
            },
            AtomFieldKind::Conditional(cond) => {
                assert!(
                    self.endianess.is_none(),
                    "endianess for a conditional field is not supported."
                );
                let enum_name =
                    if let GroupedConditionals::Enum { name, .. } = cond.group().unwrap() {
                        Some(quote!(#name ::))
                    } else {
                        None
                    };

                let conditional = cond.format_conditional(
                    |name, fields, exhaustive| {
                        let payload_len = AtomFields::nested_payload_len(payload_len, fields);
                        let tag = AtomFields::nested_tag(tag, fields);
                        let sync_parse = fields.iter().filter_map(|f| {
                            f.as_sync_parse(
                                version,
                                atom_mod,
                                version_mod,
                                payload_len.as_ref(),
                                tag.as_ref(),
                                definition_endianess,
                            )
                        });
                        let collection = fields.iter().filter_map(|f| f.as_collection());

                        let collection = if let Some(e) = enum_name.as_ref() {
                            quote! {
                                #mod_name:: #e #name (#mod_name::#name {
                                    _pd: ::core::marker::PhantomData,
                                    #(#collection)*
                                })
                            }
                        } else {
                            quote! {
                                #mod_name::#name {
                                    _pd: ::core::marker::PhantomData,
                                    #(#collection)*
                                }
                            }
                        };

                        let collection = if !exhaustive {
                            quote! {
                                Some(#collection)
                            }
                        } else {
                            collection
                        };

                        quote! {
                            #(#sync_parse)*
                            #collection
                        }
                    },
                    || quote!(None),
                );

                let name = &cond.field_name;
                quote! {
                    let #name = #conditional ;
                }
            }
            AtomFieldKind::FullBox { name, .. } => {
                quote! {
                    let atom_flags = {
                        #format_options
                        let bytes = <[u8; 3]>::parse(reader, &opts)?;
                        <#mod_name::#name as ::#KRATE::flags::FlagsParse<[u8; 3]>>::try_from_bits(bytes, &opts)?
                    };
                }
            }
            AtomFieldKind::Flags {
                name,
                parse_repr,
                field_name,
                ..
            } => {
                quote! {
                    let #field_name = {
                        #format_options
                        use ::#KRATE::flags::FlagsParse;
                        let flags = <#parse_repr>::parse(reader, &opts)?;
                        <#mod_name::#name>::try_from_bits(flags, &opts)?
                    };
                }
            }
            AtomFieldKind::Version { .. } => unreachable!(),
        })
    }

    pub fn as_collection(&self) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match &self.kind {
            AtomFieldKind::Struct(field, attr) => return attr.as_collection(field),
            AtomFieldKind::Children { kind, .. } => match kind {
                ChildrenKind::Atom => quote!(children,),
                ChildrenKind::Tagged { field_name, .. } => quote!(#field_name,),
            },
            AtomFieldKind::Conditional(cond) => {
                let name = &cond.field_name;
                quote! {
                    #name,
                }
            }
            AtomFieldKind::FullBox { .. } => {
                quote! {
                    atom_flags,
                }
            }
            AtomFieldKind::Flags { field_name, .. } => {
                let field_name = field_name
                    .clone()
                    .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));

                quote! {
                    #field_name,
                }
            }
            AtomFieldKind::Version { .. } => return None,
        })
    }

    pub fn as_inline_definition(
        &self,
        version: Option<&syn::LitInt>,
        attrs: &[syn::Attribute],
        generics: &syn::Generics,

        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&PayloadLength>,
        tag: Option<&Tag>,
        definition_endianess: Option<&Endianess>,
    ) -> Option<proc_macro2::TokenStream> {
        use quote::quote;
        Some(match &self.kind {
            AtomFieldKind::Struct(_, _) => return None,
            AtomFieldKind::Children { children, kind, .. } => {
                let children_name = match kind {
                    ChildrenKind::Atom => quote::format_ident!("Child"),
                    ChildrenKind::Tagged { tagged_name, .. } => tagged_name.clone(),
                };

                let async_children_name = quote::format_ident!("Async{}Parse", children_name);
                let (header_parse, parse_impl) = match kind {
                    ChildrenKind::Atom => {
                        let fcc_ty = syn::parse_quote!(::#KRATE::fourcc::FourCC);
                        let tag_bindings =
                            ChildList::bind_overridden_tag_consts(&children, version, &fcc_ty);

                        let sync_parse = ChildList::sync_parse(
                            &children,
                            version,
                            &fcc_ty,
                            None,
                            &quote::format_ident!("Child"),
                            true,
                            false,
                        );
                        let async_parse = ChildList::async_parse(
                            &children,
                            version,
                            &syn::parse_quote!(::#KRATE::fourcc::FourCC),
                            None,
                            &quote::format_ident!("Self"),
                            true,
                        );

                        let tag_ty = tag
                            .map(|tag| tag.ty.clone())
                            .unwrap_or(syn::parse_quote!(::#KRATE::fourcc::FourCC));
                        let async_sm =
                            ChildList::async_statemachine(&children, version, true, &tag_ty);
                        let borrow_reader_variants =
                            ChildList::borrow_reader_variants(&children, version, true);
                        (
                            Some(quote! {
                                Self::AtomHeader(mut header) => {
                                    match ::core::pin::Pin::new(&mut header).poll(cx) {
                                        ::core::task::Poll::Pending => {
                                            *self = Self::AtomHeader(header);
                                            return ::core::task::Poll::Pending;
                                        }
                                        ::core::task::Poll::Ready(res) => {
                                            let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(header);

                                            let atom = match res {
                                                Ok(atom) => atom,
                                                Err(e) => {
                                                    let max_offset = reader.offset();
                                                    let reader = ::#KRATE::reader::TrailingReader::new(reader, max_offset);
                                                    *self = Self::Done(reader, opts);
                                                    return ::core::task::Poll::Ready(Err(e));
                                                }
                                            };

                                            let atom_size = match atom.size.size()
                                                .map(|size| size.try_into())
                                                .transpose()
                                                .map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError)) {
                                                    Ok(size) => size,
                                                    Err(e) => {
                                                        let max_offset = reader.offset();
                                                        let reader = ::#KRATE::reader::TrailingReader::new(reader, max_offset);
                                                        *self = Self::Done(reader, opts);
                                                        return ::core::task::Poll::Ready(Err(e));
                                                    }
                                                }
                                                .unwrap_or(reader.remaining_size());

                                            let max_offset = reader.offset() + atom_size;
                                            let reader = ::#KRATE::reader::TrailingReader::new(reader, max_offset);

                                            #(#tag_bindings)*

                                            *self = match atom.fcc {
                                                #(#async_parse)*
                                                u => {
                                                    let extent = #KRATE::reader::PollReader::extent(&reader);
                                                    let rest = ::#KRATE::reader::PollReader::remaining_size(&reader);
                                                    Self::Seek(Child::Unsupported(::#KRATE::children::UnknownChild::new(u, extent)), ::#KRATE::reader::AsyncSeek::seek(reader, rest), opts)
                                                }
                                            };
                                        }
                                    }
                                }
                            }),
                            quote! {
                                impl <__Offset: ::#KRATE::reader::Offset> ::#KRATE::parse::Parse<__Offset> for Child {
                                    fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>, __Options: ::#KRATE::parse_options::Options>(reader: &mut __Reader, opts: __Options) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                                        let atom = ::#KRATE::atom_header::AtomHeader::parse(reader, &opts)?;
                                        let atom_size = atom.size.size()
                                            .map(|size| size.try_into())
                                            .transpose().map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError))?
                                            .unwrap_or(reader.remaining_size());

                                        let max_offset = reader.offset() + atom_size;
                                        let mut reader = ::#KRATE::reader::TrailingReader::new(&mut*reader, max_offset);

                                        #(#tag_bindings)*

                                        Ok(match atom.fcc {
                                            #(#sync_parse)*
                                            missed => {
                                                let extent = ::#KRATE::reader::Reader::extent(&reader);
                                                ::#KRATE::reader::Reader::seek_remaining(&mut reader)?;
                                                Child::Unsupported(::#KRATE::children::UnknownChild::new(missed, extent))
                                            }
                                        })
                                    }
                                }

                                pub enum AsyncChildParse<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> {
                                    AtomHeader(<::#KRATE::atom_header::AtomHeader as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<__Options, __Reader>),
                                    #(#async_sm)*
                                    Seek(Child<__Offset>, ::#KRATE::reader::AsyncSeek<__Offset, ::#KRATE::reader::TrailingReader<__Offset, __Reader>>, __Options),
                                    Done(::#KRATE::reader::TrailingReader<__Offset, __Reader>, __Options),
                                    Empty,
                                }

                                impl <__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin> ::#KRATE::parse::AsyncParse<__Offset> for Child {
                                    type Fut<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = AsyncChildParse<__Options, __Offset, __Reader>;
                                    fn create_fut<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: __Options) -> Self::Fut<__Options, __Reader> {
                                        AsyncChildParse::AtomHeader(::#KRATE::atom_header::AtomHeader::create_fut(reader, opts))
                                    }
                                }

                                impl <__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> AsyncChildParse<__Options, __Offset, __Reader> {
                                    pub fn reader(&mut self) -> &mut ::#KRATE::reader::TrailingReader<__Offset, __Reader> {
                                        match self {
                                            Self::Done(reader, _) => reader,
                                            _ => unreachable!("should not be able to access mid future")
                                        }
                                    }
                                }

                                impl <__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<__Options, __Reader> for AsyncChildParse<__Options, __Offset, __Reader> {
                                    fn take_reader(self) -> (__Reader, __Options) {
                                        match self {
                                            Self::Done(reader, opts) => (reader.reader, opts),
                                            _ => unreachable!("invalid state of AsyncChildParse")
                                        }
                                    }
                                    fn borrow_reader(&mut self) -> (&mut __Reader, &__Options) {
                                        match self {
                                            Self::AtomHeader(fut) => fut.borrow_reader(),
                                            #(#borrow_reader_variants)*
                                            Self::Seek(_, seek, opts) => (&mut seek.reader.reader, opts),
                                            Self::Done(reader, opts) => (&mut reader.reader, opts),
                                            Self::Empty => unreachable!(),
                                        }
                                    }
                                }
                            },
                        )
                    }
                    ChildrenKind::Tagged { skip_seek, .. } => {
                        let tag_ty = tag
                            .map(|tag| tag.ty.clone())
                            .unwrap_or(syn::parse_quote!(::#KRATE::fourcc::FourCC));

                        let async_children_name_with_len =
                            quote::format_ident!("{async_children_name}WithLen");
                        let async_sm =
                            ChildList::async_statemachine(&children, version, false, &tag_ty);
                        let async_sm_with_len =
                            ChildList::async_statemachine(&children, version, false, &tag_ty);
                        let borrow_reader_variants =
                            ChildList::borrow_reader_variants(&children, version, false);
                        let borrow_reader_variants_with_len =
                            ChildList::borrow_reader_variants(&children, version, true);
                        let sm_poll = ChildList::async_statemachine_poll(
                            &children,
                            version,
                            &children_name,
                            *skip_seek,
                        );
                        (
                            None,
                            quote! {
                                pub enum #async_children_name<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> {
                                    #(#async_sm)*
                                    Seek(<#children_name as #KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::reader::AsyncSeek<__Offset, __Reader>, __Options),
                                    Done(__Reader, __Options),
                                    Empty,
                                }

                                pub enum #async_children_name_with_len<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> {
                                    #(#async_sm_with_len)*
                                    Seek(<#children_name as #KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::reader::AsyncSeek<__Offset, __Reader>, __Options),
                                    Done(__Reader, __Options),
                                    Empty,
                                }

                                impl <__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<__Options, __Reader> for #async_children_name<__Options, __Offset, __Reader> {
                                    fn take_reader(self) -> (__Reader, __Options) {
                                        match self {
                                            Self::Done(reader, opts) => (reader, opts),
                                            _ => unreachable!("invalid state of AsyncChildParse")
                                        }
                                    }
                                    fn borrow_reader(&mut self) -> (&mut __Reader, &__Options) {
                                        match self {
                                            #(#borrow_reader_variants)*
                                            Self::Seek(_, seek, opts) => (&mut seek.reader, opts),
                                            Self::Done(reader, opts) => (reader, opts),
                                            Self::Empty => unreachable!(),
                                        }
                                    }
                                }

                                impl <__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> ::#KRATE::reader::TakeReader<__Options, __Reader> for #async_children_name_with_len<__Options, __Offset, ::#KRATE::reader::TrailingReader<__Offset, __Reader>> {
                                    fn take_reader(self) -> (__Reader, __Options) {
                                        match self {
                                            Self::Done(reader, opts) => (reader.reader, opts),
                                            _ => unreachable!("invalid state of AsyncChildParse")
                                        }
                                    }
                                    fn borrow_reader(&mut self) -> (&mut __Reader, &__Options) {
                                        match self {
                                            #(#borrow_reader_variants_with_len)*
                                            Self::Seek(_, seek, opts) => (&mut seek.reader.reader, opts),
                                            Self::Done(reader, opts) => (&mut reader.reader, opts),
                                            Self::Empty => unreachable!(),
                                        }
                                    }
                                }

                                impl <__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin> ::#KRATE::parse::AsyncParse<__Offset> for #children_name {
                                    type Fut<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = #async_children_name<__Options, __Offset, __Reader>;
                                    fn create_fut<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: __Options) -> Self::Fut<__Options, __Reader> {
                                        unreachable!("need tag to parse")
                                    }
                                }

                                impl <__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> Future for #async_children_name_with_len<__Options, __Offset, ::#KRATE::reader::TrailingReader<__Offset, __Reader>> {
                                    type Output = ::core::result::Result<<#children_name as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError>;
                                    fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                                        loop {
                                            let mut this = ::core::mem::replace(&mut*self, Self::Empty);
                                            match this {
                                                #(#sm_poll)*
                                                Self::Seek(child, mut fut, opts) => {
                                                    match ::core::pin::Pin::new(&mut fut).poll(cx) {
                                                        ::core::task::Poll::Pending => {
                                                            *self = Self::Seek(child, fut, opts);
                                                            return ::core::task::Poll::Pending;
                                                        }
                                                        ::core::task::Poll::Ready(res) => {
                                                            let reader = fut.reader;
                                                            *self = Self::Done(reader, opts);
                                                            let _ = res?;
                                                            return ::core::task::Poll::Ready(Ok(child))
                                                        }
                                                    }
                                                }
                                                Self::Done(..) => panic!("poll after completion"),
                                                Self::Empty => unreachable!(),
                                            }
                                        }
                                    }
                                }
                            },
                        )
                    }
                };

                let tag_ty = tag
                    .map(|tag| tag.ty.clone())
                    .unwrap_or(syn::parse_quote!(::#KRATE::fourcc::FourCC));

                let tagged_variants = ChildList::tagged_variants(&children, version, &tag_ty);

                let seek_skip = match kind {
                    ChildrenKind::Atom => false,
                    ChildrenKind::Tagged { skip_seek, .. } => *skip_seek,
                };

                let sm_poll = ChildList::async_statemachine_poll(
                    &children,
                    version,
                    &children_name,
                    seek_skip,
                );

                quote! {
                    #(#attrs)*
                    pub enum #children_name<__Offset = usize> {
                        #(#tagged_variants)*
                        Unsupported(::#KRATE::children::UnknownChild<__Offset, #tag_ty>),
                    }

                    impl ::#KRATE::parse::Parsed for #children_name {
                        type Output<__Offset> = #children_name<__Offset>;
                    }

                    impl <__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> Future for #async_children_name<__Options, __Offset, __Reader> {
                        type Output = ::core::result::Result<<#children_name as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError>;
                        fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                            loop {
                                let mut this = ::core::mem::replace(&mut*self, Self::Empty);
                                match this {
                                    #header_parse
                                    #(#sm_poll)*
                                    Self::Seek(child, mut fut, opts) => {
                                        match ::core::pin::Pin::new(&mut fut).poll(cx) {
                                            ::core::task::Poll::Pending => {
                                                *self = Self::Seek(child, fut, opts);
                                                return ::core::task::Poll::Pending;
                                            }
                                            ::core::task::Poll::Ready(res) => {
                                                let reader = fut.reader;
                                                *self = Self::Done(reader, opts);
                                                let _ = res?;
                                                return ::core::task::Poll::Ready(Ok(child))
                                            }
                                        }
                                    }
                                    Self::Done(..) => panic!("poll after completion"),
                                    Self::Empty => unreachable!(),
                                }
                            }
                        }
                    }

                    #parse_impl
                }
            }
            AtomFieldKind::Conditional(cond) => {
                let nested_defs = cond.as_async_state(
                    generics,
                    generic_collection,
                    payload_len,
                    tag,
                    definition_endianess,
                );

                match cond.group().unwrap() {
                    GroupedConditionals::Struct { name, fields } => {
                        let payload_len = AtomFields::nested_payload_len(payload_len, &fields);
                        let tag = AtomFields::nested_tag(tag, &fields);
                        let helper_fns = crate::conditional::ConditionalVariant::format_helper_fns(
                            &name,
                            &fields,
                            generics,
                            definition_endianess,
                        );

                        let fs = fields
                            .iter()
                            .filter_map(|f| f.as_variant_decl(payload_len.as_ref(), generics));

                        let definitions = fields.iter().filter_map(|field| {
                            field.as_inline_definition(
                                version,
                                attrs,
                                generics,
                                generic_collection,
                                payload_len.as_ref(),
                                tag.as_ref(),
                                definition_endianess,
                            )
                        });

                        let mut generics = generics.clone();

                        for param in &mut generics.params {
                            if let syn::GenericParam::Type(ty) = param {
                                ty.bounds.push(syn::parse_quote!(::#KRATE::parse::Parsed));
                            }
                        }

                        generics.params.push(syn::parse_quote!(__Offset = usize));
                        let (_, _, where_clause) = generics.split_for_impl();

                        let pd_generics = generics.params.iter().filter_map(|p| match p {
                            syn::GenericParam::Type(t) => Some(&t.ident),
                            _ => None,
                        });

                        quote! {
                            #nested_defs

                            #(#definitions)*

                            #(#attrs)*
                            pub struct #name #generics #where_clause {
                                pub(super) _pd: core::marker::PhantomData<( #(#pd_generics,)* )>,
                                #(#fs,)*
                            }

                            #helper_fns
                        }
                    }
                    GroupedConditionals::Enum { name, variants } => {
                        let vs = variants.iter().map(|v| {
                            let name = &v.variant_name;
                            let mut generics = generics.clone();
                            generics.params.push(syn::parse_quote!(__Offset = usize));

                            let (_, ty_generics, _) = generics.split_for_impl();

                            quote! {
                                #name(#name #ty_generics)
                            }
                        });

                        let variant_defs = variants.iter().map(|v| {
                            let name = &v.variant_name;
                            let payload_len = AtomFields::nested_payload_len(payload_len, &v.fields);
                            let helper_fns = crate::conditional::ConditionalVariant::format_helper_fns(&v.variant_name, &v.fields, generics, definition_endianess);

                            let fs = v.fields.iter().filter_map(|f| f.as_variant_decl(payload_len.as_ref(), generics));

                            let mut generics = generics.clone();
                            generics.params.push(syn::parse_quote!(__Offset = usize));

                            let (_, _, where_clause) = generics.split_for_impl();
                            let pd_generics = generics.params.iter().filter_map(|p| {
                                match p {
                                    syn::GenericParam::Type(t) => Some(&t.ident),
                                    _ => None
                                }
                            });

                            quote! {
                                #(#attrs)*
                                pub struct #name #generics #where_clause {
                                    pub(super) _pd: ::core::marker::PhantomData <( #(#pd_generics,)* )>,
                                    #(#fs,)*
                                }

                                #helper_fns
                            }
                        });

                        let field_defs = variants.iter().flat_map(|v| {
                            let payload_len =
                                AtomFields::nested_payload_len(payload_len, &v.fields);
                            let tag = AtomFields::nested_tag(tag, &v.fields);
                            v.fields.iter().filter_map(move |f| {
                                f.as_inline_definition(
                                    version,
                                    attrs,
                                    generics,
                                    generic_collection,
                                    payload_len.as_ref(),
                                    tag.as_ref(),
                                    definition_endianess,
                                )
                            })
                        });

                        let mut generics = generics.clone();
                        generics.params.push(syn::parse_quote!(__Offset = usize));
                        let (_, _, where_clause) = generics.split_for_impl();
                        quote! {
                            #nested_defs

                            #(#field_defs)*

                            #(#variant_defs)*

                            #(#attrs)*
                            pub enum #name #generics #where_clause {
                                #(#vs,)*
                            }
                        }
                    }
                }
            }
            AtomFieldKind::FullBox { name, flags } => {
                let can_elide_flags = FlagList::can_elide_flag_storage(&flags, version);
                let flags_debug = FlagList::debug_impl(flags, name, version, can_elide_flags);

                let repr = syn::parse_quote!(u32);
                let flags_check = FlagList::flags_parse_check(flags, version);
                let flags_impl = FlagList::flags_impl(name, flags, &repr, version);
                let flag_storage_elision =
                    can_elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                let bitops_impl = FlagList::flags_bitops_impl(name);
                let async_parse_repr = syn::parse_quote!([u8; 3]);
                let async_parse_impl = FlagList::async_parse_impl(
                    name,
                    &async_parse_repr,
                    definition_endianess,
                    self.endianess.as_ref(),
                );

                let to_bits_impl = FlagList::to_bits_impl_fullbox(&flags, version, can_elide_flags);
                let bits_impl = FlagList::bits_impl(name, &syn::parse_quote!(u32));

                quote! {
                    #[derive(Clone, Copy, PartialEq, Eq)]
                    pub struct #name {
                        #flag_storage_elision
                        inner: [u8; 3],
                    }

                    impl #name {
                        #(#flags_impl)*
                        pub fn from_bytes(bytes: [u8; 3]) -> Self {
                            Self {
                                #flag_storage_elision
                                inner: bytes,
                            }
                        }

                        const fn from_bits_(bits: u32) -> Self {
                            let [b0, b1, b2, _] = bits.to_be_bytes();
                            Self {
                                #flag_storage_elision
                                inner: [b0, b1, b2]
                            }
                        }
                        const fn to_bits_(self) -> u32 {
                            #to_bits_impl
                        }
                    }

                    impl ::#KRATE::flags::FlagsParse<[u8; 3]> for #name {
                        fn try_from_bits<__Options: ::#KRATE::parse_options::Options>(bits: [u8; 3], opts: __Options) -> ::core::result::Result<Self, ::#KRATE::error::ParseError> {
                            {
                                let [b0, b1, b2] = bits;
                                let bits = u32::from_be_bytes([b0, b1, b2, 0]);
                                #flags_check
                            }

                            Ok(Self {
                                #flag_storage_elision
                                inner: bits,
                            })
                        }
                    }

                    #bits_impl
                    #async_parse_impl

                    #bitops_impl
                    #flags_debug
                }
            }
            AtomFieldKind::Flags {
                name,
                parse_repr,
                flags,
                ..
            } => {
                let can_elide_flags = FlagList::can_elide_flag_storage(&flags, version);
                let flags_debug = FlagList::debug_impl(flags, name, version, can_elide_flags);
                let flags_trait_impl =
                    FlagList::flags_trait_impl(name, flags, parse_repr, version, can_elide_flags);

                let bitops_impl = FlagList::flags_bitops_impl(name);
                let async_parse_impl = FlagList::async_parse_impl(
                    name,
                    parse_repr,
                    definition_endianess,
                    self.endianess.as_ref(),
                );

                let flags_impl = FlagList::flags_impl(name, flags, parse_repr, version);
                let flag_storage_elision =
                    can_elide_flags.then(|| quote! { #[cfg(feature = "store_unknown_fields")] });

                let to_bits_impl = FlagList::to_bits_impl(&flags, version, can_elide_flags);
                let bits_impl = FlagList::bits_impl(name, parse_repr);

                quote! {
                    #[derive(Clone, Copy, PartialEq, Eq)]
                    pub struct #name {
                        #flag_storage_elision
                        inner: #parse_repr,
                    }

                    impl #name {
                        #(#flags_impl)*

                        const fn from_bits_(bits: #parse_repr) -> Self {
                            Self {
                                #flag_storage_elision
                                inner: bits,
                            }
                        }

                        const fn to_bits_(self) -> #parse_repr {
                            #to_bits_impl
                        }
                    }

                    #flags_trait_impl
                    #flags_debug
                    #bitops_impl
                    #async_parse_impl
                    #bits_impl
                }
            }
            AtomFieldKind::Version { .. } => return None,
        })
    }
}

#[derive(Clone)]
pub enum StructFieldAttr {
    // definition of repeated fields `Entry` with length of type `Size`
    //
    // #[dynamic_array(size_type = Size, zero_relative = false)]
    // some_field: Entry,
    //
    DynamicArray {
        zero_relative: syn::LitBool,
        length_ty: syn::Type,
    },
    // declaration of a field with an expected val
    // the general case of a reserved field
    //
    // #[expected(4)]
    // payload_size: u8,
    //
    // will expand to
    //
    // let payload_size = /* parse impl */;
    // if payload_size != 4 {
    //      return Err(..)
    // }
    Expected {
        val: syn::Expr,
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
    TrailingArray {
        // specifies the length from other fields or a constant expr
        //
        // #[trailing_array(u8, 8)]
        // or
        // #[trailing_array(u32, some_other_field * 2)]
        from_expr: Option<LenFromExpr>,
    },
    // declaration of a null terminated array
    //
    // #[null_terminated_array]
    // null_terminated: u32
    NullTerminatedArray,
    // declaration of the length of the trailing payload
    // at the moment it only supports non-inclusive length
    // e.g, if a headers length includes itself,
    // it does not calculate the offset
    PayloadLength {
        // specifies an offset from the length given, e.g if a headers
        // length includes itself
        //
        // #[payload_length(offset = 8)]
        offset: Option<syn::Expr>,
        // specifies the padding applied to the payload,
        // e.g if its padded to an 2 byte boundary, 4 byte boundary, etc.
        //
        // #[payload_length(padding = 2)]
        padding: Option<syn::LitInt>,
    },
    // declaration of a tag, which will be used in a #[tagged] field
    // to parse a variant with the given tag
    Tag,
    // definition of trailing payload at the end of the atom
    //
    // #[payload]
    // some_field: [u8],
    Payload {
        // specifies the length from other fields or a constant expr
        //
        // #[payload(u8, 8)]
        // or
        // #[payload(u32, some_other_field * 2)]
        from_expr: Option<LenFromExpr>,
    },
    // definition of a padded field,
    // e.g, a field which may have padding / data trailing it
    // _requires_ either #[payload_length] or a length specifier
    //
    // #[payload_length]
    // section_length: u32,
    // #[padded]
    // last_known_field: u8,
    Padded {
        // specifies the length from other fields or a constant expr
        //
        // #[padded(u8, 8)]
        // or
        // #[padded(u32, some_other_field * 2)]
        from_expr: Option<LenFromExpr>,
    },
    // the field that identifies the version number
    //
    // #[version]
    // version: u8,
    VersionIdentifier,
    // specifies that this field is used for a condition
    // (which may be stored if it is reused)
    Condition,
    // specifies that the field of the same name marked as #[condition] is used
    // e.g, the field is stored but not parsed in the conditional
    ReuseCondition,
    // just a normal field
    //
    // field: u8,
    Normal,
}

impl StructFieldAttr {
    pub fn parse_from_syn(attr: &syn::Attribute) -> syn::parse::Result<Self> {
        let path = attr.path();
        Ok(if path.is_ident("dynamic_array") {
            let mut size_ty = None;
            let mut zero_relative = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("size_type") {
                    size_ty = Some(
                        meta.value()
                            .expect("no ty for size_type")
                            .parse::<syn::Type>()
                            .expect("could not parse type of size_type"),
                    );
                    Ok(())
                } else if meta.path.is_ident("zero_relative") {
                    zero_relative = Some(
                        meta.value()
                            .expect("no value for zero_relative")
                            .parse::<syn::LitBool>()
                            .expect("could not parse bool from zero_relative"),
                    );
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
        } else if path.is_ident("expected") {
            let val = attr.parse_args::<syn::Expr>()?;
            Self::Expected { val }
        } else if path.is_ident("null_terminated_string") {
            Self::NullTerminatedString
        } else if path.is_ident("pascal_string") {
            let length_ty = attr.parse_args::<syn::Type>()?;
            Self::PascalString { length_ty }
        } else if path.is_ident("trailing_array") {
            let from_expr = LenFromExpr::parse_from_attr(attr)?;
            Self::TrailingArray { from_expr }
        } else if path.is_ident("null_terminated_array") {
            Self::NullTerminatedArray
        } else if path.is_ident("padded") {
            let from_expr = LenFromExpr::parse_from_attr(attr)?;
            Self::Padded { from_expr }
        } else if path.is_ident("payload") {
            let from_expr = LenFromExpr::parse_from_attr(attr)?;
            Self::Payload { from_expr }
        } else if path.is_ident("payload_length") {
            use syn::Meta;
            let (offset, padding) = match &attr.meta {
                Meta::Path(_) => (None, None),
                Meta::List(list) => {
                    let parser = |input: syn::parse::ParseStream| {
                        let mut offset = None;
                        let mut padding = None;
                        while !input.is_empty() {
                            let name: syn::Path = input.parse()?;
                            let _: syn::Token![=] = input.parse()?;
                            if name.is_ident("offset") {
                                offset = Some(input.parse()?);
                            } else if name.is_ident("padding") {
                                padding = Some(input.parse()?);
                            } else {
                                panic!(
                                    "only `offset` and `padding` meta are supported in the payload_length attr"
                                );
                            }
                            let _: Option<syn::Token![,]> = input.parse()?;
                        }
                        Ok((offset, padding))
                    };
                    use syn::parse::Parser;
                    parser.parse2(list.tokens.clone())?
                }
                Meta::NameValue(_) => panic!("name-value pair for conditional is not supported"),
            };

            Self::PayloadLength { offset, padding }
        } else if path.is_ident("tag") {
            Self::Tag
        } else if path.is_ident("version") {
            Self::VersionIdentifier
        } else if path.is_ident("condition") {
            Self::Condition
        } else if path.is_ident("reuse_condition") {
            Self::ReuseCondition
        } else {
            unreachable!("unknown attr");
        })
    }

    pub fn as_sync_parse(
        &self,
        field: &syn::Field,
        payload_len: Option<&PayloadLength>,
        format_options: &proc_macro2::TokenStream,
    ) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        use quote::quote;
        Some(match self {
            Self::Reserved => {
                quote! {
                    {
                        #format_options
                        let _reserved = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?;
                        if ::#KRATE::parse_options::Options::options(&opts).error_on_used_reserved_fields && (_reserved != unsafe { ::core::mem::zeroed::<#ty>() }) {
                            return Err(::#KRATE::error::ParseError::UsedReservedField);
                        }
                    }
                }.into()
            },
            Self::Expected {
                val,
            } => {
                quote! {
                    {
                        #format_options
                        let _expected = <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?;
                        if _expected != #val {
                            return Err(::#KRATE::error::ParseError::UnexpectedValue);
                        }
                    }
                }.into()
            }
            Self::DynamicArray {
                length_ty,
                zero_relative,
            } => {
                quote! {
                    let #name = {
                        #format_options
                        <::#KRATE::array::DynamicArray::<__Offset, #length_ty, #ty, #zero_relative> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }
            }
            Self::NullTerminatedArray => {
                quote! {
                    let #name = {
                        #format_options
                        <::#KRATE::array::NullTerminated::<__Offset, #ty> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }
            }
            Self::TrailingArray { from_expr } => {
                if let Some(LenFromExpr { size_expr, .. }) = from_expr {
                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::trailing::Trailing::parse_from_len(#size_expr, reader, &opts)?
                        };
                    }
                } else if let Some(payload_len) = payload_len {
                    let len = payload_len.as_parsed_length();

                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::trailing::Trailing::parse_from_len(#len, reader, &opts)?
                        };
                    }
                } else {
                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::trailing::Trailing::parse(reader, &opts)?
                        };
                    }
                }
            }
            Self::Tag => {
                quote! {
                    let _tag = {
                        #format_options
                        <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }
            }
            Self::PayloadLength{..} => {
                quote! {
                    let _payload_len = {
                        #format_options
                        <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }
            }
            Self::Payload { from_expr } => {
                if let Some(LenFromExpr { size_expr, .. }) = from_expr {
                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::payload::Payload::parse_from_len(#size_expr, reader, &opts)?
                        };
                    }
                } else if let Some(payload_len) = payload_len {
                    let len = payload_len.as_parsed_length();

                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::payload::Payload::parse_from_len(#len, reader, &opts)?
                        };
                    }
                } else {
                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::payload::Payload::parse(reader, &opts)?
                        };
                    }
                }
            }
            Self::Padded { from_expr } => {
                if let Some(LenFromExpr { size_expr, ty: size_ty }) = from_expr {
                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::padded::Padded::<__Offset, #size_ty, #ty>::parse_from_len(#size_expr, reader, &opts)?
                        };
                    }
                } else if let Some(payload_len) = payload_len {
                    let len = payload_len.as_parsed_length();
                    let size_ty = &payload_len.ty;

                    quote! {
                        let #name = {
                            #format_options
                            ::#KRATE::padded::Padded::<__Offset, #size_ty, #ty>::parse_from_len(#len, reader, &opts)?
                        };
                    }
                } else {
                    panic!("neither payload length or length specifier specified for padded field")
                }
            }
            Self::Normal | Self::Condition => {
                quote! {
                    let #name = {
                        #format_options
                        <#ty as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }.into()
            }
            Self::PascalString {
                length_ty
            } => {
                quote! {
                    let #name = {
                        #format_options
                        <::#KRATE::string::PascalString::<__Offset, #length_ty> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    let #name = {
                        #format_options
                        <::#KRATE::string::NullTerminatedString<__Offset> as ::#KRATE::parse::Parse<__Offset>>::parse(reader, &opts)?
                    };
                }
            }
            // builtin parsing in versioned impl
            Self::VersionIdentifier => return None,
            // moved from a field that was already parsed
            Self::ReuseCondition => return None,
        })
    }

    pub fn as_collection(&self, field: &syn::Field) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        use quote::quote;
        Some(match self {
            Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::PayloadLength { .. }
            | Self::Tag
            | Self::Condition => {
                return None;
            }
            Self::Normal
            | Self::DynamicArray { .. }
            | Self::TrailingArray { .. }
            | Self::Payload { .. }
            | Self::Padded { .. }
            | Self::PascalString { .. }
            | Self::ReuseCondition
            | Self::NullTerminatedArray
            | Self::NullTerminatedString => quote! {
                #name,
            }
            .into(),
        })
    }

    pub fn as_field_decl(
        &self,
        field: &syn::Field,
        payload_len: Option<&PayloadLength>,
    ) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;

        use quote::quote;
        Some(match self {
            Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::PayloadLength { .. }
            | Self::Tag
            | Self::Condition
            | Self::ReuseCondition => return None,
            Self::Normal | Self::Padded { .. } => {
                quote! {
                    pub #name: <#ty as ::#KRATE::parse::Parsed>::Output<__Offset>
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    pub #name: ::#KRATE::array::DynamicArray<__Offset, #length_ty, #ty, #zero_relative>
                }
            }
            Self::NullTerminatedArray => {
                quote! {
                    pub #name: ::#KRATE::array::NullTerminated<__Offset, #ty>
                }
            }
            Self::TrailingArray { from_expr } => {
                if let Some(LenFromExpr { ty: len_ty, .. }) = from_expr {
                    quote! {
                        pub #name: ::#KRATE::trailing::Trailing<__Offset, #ty, #len_ty>
                    }
                } else if let Some(PayloadLength { ty: len_ty, .. }) = payload_len {
                    quote! {
                        pub #name: ::#KRATE::trailing::Trailing<__Offset, #ty, #len_ty>
                    }
                } else {
                    quote! {
                        pub #name: ::#KRATE::trailing::Trailing<__Offset, #ty>
                    }
                }
            }
            Self::Payload { from_expr } => {
                if let Some(LenFromExpr { ty: len_ty, .. }) = from_expr {
                    quote! {
                        pub #name: ::#KRATE::payload::Payload<__Offset, #len_ty>
                    }
                } else if let Some(PayloadLength { ty: len_ty, .. }) = payload_len {
                    quote! {
                        pub #name: ::#KRATE::payload::Payload<__Offset, #len_ty>
                    }
                } else {
                    quote! {
                        pub #name: ::#KRATE::payload::Payload<__Offset>
                    }
                }
            }
            Self::PascalString { length_ty } => {
                quote! {
                    pub #name: ::#KRATE::string::PascalString<__Offset, #length_ty>
                }
            }
            Self::NullTerminatedString => {
                quote! {
                    pub #name: ::#KRATE::string::NullTerminatedString<__Offset>
                }
            }
        })
    }

    pub fn as_sync_helper_fn(
        &self,
        field: &syn::Field,
        format_options: &proc_macro2::TokenStream,
        options_ty: &proc_macro2::TokenStream,
    ) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        let name = name.as_ref().unwrap();

        use quote::quote;
        Some(match self {
            Self::Normal
            | Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::Tag
            | Self::PayloadLength { .. }
            | Self::Payload { .. }
            | Self::Padded { .. }
            | Self::Condition
            | Self::ReuseCondition
            | Self::PascalString { .. }
            | Self::NullTerminatedString => {
                return None;
            }
            Self::TrailingArray { .. } => {
                quote! {
                    pub fn #name<__Options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: __Reader, opts: __Options) -> ::core::result::Result<::#KRATE::trailing::TrailingIterator<#options_ty, __Offset, __Reader, #ty>, ::#KRATE::error::ParseError> {
                        #format_options
                        ::#KRATE::trailing::TrailingIterator::from_trailing(&self.#name, reader, opts)
                    }
                }
            }
            Self::NullTerminatedArray => {
                quote! {
                    pub fn #name<__Options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::trailing::TrailingIterator<#options_ty, __Offset, __Reader, #ty> {
                        #format_options
                        ::#KRATE::trailing::TrailingIterator::from_null_terminated(&self.#name, reader, opts)
                    }
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                quote! {
                    pub fn #name<__Options, __Reader: ::#KRATE::reader::Reader<__Offset>>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::array::DynamicArrayIter<#options_ty, __Offset, __Reader, #length_ty, #ty, #zero_relative> {
                        #format_options
                        ::#KRATE::array::DynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }
                }
            }
        })
    }

    pub fn as_async_helper_fn(
        &self,
        field: &syn::Field,
        format_options: &proc_macro2::TokenStream,
        options_ty: &proc_macro2::TokenStream,
    ) -> Option<proc_macro2::TokenStream> {
        let name = &field.ident;
        let ty = &field.ty;
        let name = name.as_ref().unwrap();

        use quote::quote;
        Some(match self {
            Self::Normal
            | Self::Reserved
            | Self::Expected { .. }
            | Self::VersionIdentifier
            | Self::Tag
            | Self::PayloadLength { .. }
            | Self::Payload { .. }
            | Self::Padded { .. }
            | Self::Condition
            | Self::ReuseCondition
            | Self::PascalString { .. }
            | Self::NullTerminatedString => {
                return None;
            }
            Self::TrailingArray { .. } => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #async_name<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: __Reader, opts: __Options) -> ::core::result::Result<::#KRATE::trailing::AsyncTrailingIterator<#options_ty, __Offset, __Reader, #ty>, ::#KRATE::error::ParseError> {
                        #format_options
                        ::#KRATE::trailing::AsyncTrailingIterator::from_trailing(&self.#name, reader, opts)
                    }
                }
            }
            Self::NullTerminatedArray => {
                let async_name = quote::format_ident!("{name}_async");
                quote! {
                    pub fn #async_name<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::trailing::AsyncTrailingIterator<#options_ty, __Offset, __Reader, #ty> {
                        #format_options
                        ::#KRATE::trailing::AsyncTrailingIterator::from_null_terminated(&self.#name, reader, opts)
                    }
                }
            }
            Self::DynamicArray {
                zero_relative,
                length_ty,
            } => {
                let async_name = quote::format_ident!("{name}_async");

                quote! {
                    pub fn #async_name<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::SwapOffsets<__Offset> + ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(&self, reader: __Reader, opts: __Options) -> ::#KRATE::array::AsyncDynamicArrayIter<#options_ty, __Offset, __Reader, #length_ty, #ty, #zero_relative> {
                        #format_options
                        ::#KRATE::array::AsyncDynamicArrayIter::from_dynamic_array(&self.#name, reader, opts)
                    }
                }
            }
        })
    }
}

pub fn format_parse_impl(
    name: &syn::Ident,
    sync_parsing: &[proc_macro2::TokenStream],
    field_collection: &[proc_macro2::TokenStream],
    generics: &syn::Generics,
    generic_collection: Option<&proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let mut generics = generics.clone();

    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds
                .push(syn::parse_quote!(::#KRATE::parse::Parse<__Offset>));
        }
    }

    let parsed_generics = generics.clone();

    generics
        .params
        .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset = usize));

    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, parse_generics, _) = parsed_generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ::#KRATE::parse::Parse<__Offset> for #name #parse_generics #where_clause {
            fn parse<__Reader: ::#KRATE::reader::Reader<__Offset>, __Options: ::#KRATE::parse_options::Options>(reader: &mut __Reader, opts: __Options) -> ::core::result::Result<<Self as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError> {
                #(#sync_parsing)*
                Ok(#name {
                    #(#field_collection)*
                    _offset: ::core::marker::PhantomData,
                    #generic_collection
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChildrenKind {
    Atom,
    Tagged {
        len_from_expr: Option<LenFromExpr>,
        field_name: syn::Ident,
        tagged_name: syn::Ident,
        skip_seek: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Tag {
    ty: syn::Type,
}

#[derive(Clone, Debug)]
pub struct PayloadLength {
    ty: syn::Type,
    offset: Option<syn::Expr>,
    padding: Option<syn::LitInt>,
}

impl PayloadLength {
    fn as_parsed_length(&self) -> proc_macro2::TokenStream {
        use quote::quote;
        match (&self.offset, &self.padding) {
            (Some(offset), Some(padding)) => {
                quote!(
                    {
                        const _: () = assert!(#padding > 1);

                        _payload_len
                            .checked_sub(#offset)
                            .ok_or(::#KRATE::error::ParseError::AtomSizeTooSmall)?
                            .checked_next_multiple_of(#padding)
                            .ok_or(::#KRATE::error::ParseError::AtomSizeTooLarge)?
                    }
                )
            }
            (Some(offset), None) => {
                quote!(
                    _payload_len
                        .checked_sub(#offset)
                        .ok_or(::#KRATE::error::ParseError::AtomSizeTooSmall)?
                )
            }
            (None, Some(padding)) => {
                quote!(
                    {
                        const _: () = assert!(#padding > 1);
                        _payload_len
                            .checked_next_multiple_of(#padding)
                            .ok_or(::#KRATE::error::ParseError::AtomSizeTooLarge)?
                    }
                )
            }
            (None, None) => {
                quote!(_payload_len)
            }
        }
    }
}

// for fields that use different readers in their sms
// based on whether or not length is present (e.g Tagged)
fn reader_from_lengths(
    len_from_expr: Option<&LenFromExpr>,
    payload_len: Option<&PayloadLength>,
) -> proc_macro2::TokenStream {
    use quote::quote;
    if len_from_expr.is_some() || payload_len.is_some() {
        let len = match (len_from_expr, payload_len) {
            (Some(LenFromExpr { size_expr, .. }), _) => {
                quote! {
                    (#size_expr)
                }
            }
            (_, Some(payload_len)) => payload_len.as_parsed_length(),
            _ => unreachable!(),
        };

        quote! {
            {
                let offset = reader.offset();
                let size: __Offset = #len
                    .try_into()
                    .map_err(|_| ::#KRATE::error::ParseError::IntegerConversion(::#KRATE::error::TryFromIntError))?;

                let max_offset = offset + size;
                ::#KRATE::reader::TrailingReader::new(reader, max_offset)
            }
        }
    } else {
        quote! {
            reader
        }
    }
}

#[derive(Clone, Debug)]
pub struct LenFromExpr {
    ty: syn::Type,
    size_expr: syn::Expr,
}

impl LenFromExpr {
    fn parse_from_attr(attr: &syn::Attribute) -> syn::Result<Option<LenFromExpr>> {
        use syn::Meta;
        let from_expr = match &attr.meta {
            Meta::Path(_) => None,
            Meta::List(list) => {
                let parser = Self::parse_from_syn;
                use syn::parse::Parser;
                Some(parser.parse2(list.tokens.clone())?)
            }
            Meta::NameValue(_) => panic!("name-value pair for conditional is not supported"),
        };
        Ok(from_expr)
    }

    fn parse_from_syn(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ty = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let size_expr = input.parse()?;
        Ok(Self { ty, size_expr })
    }
}

pub enum FieldKind<'a> {
    PayloadLength,
    Payload(Option<&'a LenFromExpr>),
    Trailing(Option<&'a LenFromExpr>),
    Padded(Option<&'a LenFromExpr>),
    Reserved,
    Expected(&'a syn::Expr),
    Regular,
    Conditional(&'a Conditionals),
    Condition,
    Tag,
    Tagged(Option<&'a LenFromExpr>, &'a [Child], &'a syn::Ident),
}

impl FieldKind<'_> {
    fn as_next_fut(
        &self,
        next_ty: &proc_macro2::TokenStream,
        generics: &syn::Generics,
        mod_name: Option<&proc_macro2::TokenStream>,
        generic_collection: Option<&proc_macro2::TokenStream>,
        payload_len: Option<&PayloadLength>,
        tag: Option<&Tag>,
        current_field_endianess: Option<&Endianess>,
        next_field_endianess: Option<&Endianess>,
        definition_endianess: Option<&Endianess>,
        first: bool,
    ) -> proc_macro2::TokenStream {
        use quote::quote;
        let options_transition = Endianess::format_state_transition_options(
            current_field_endianess,
            next_field_endianess,
            definition_endianess,
            first,
        );
        match self {
            Self::Trailing(expr_len) => {
                if let Some(LenFromExpr { size_expr, .. }) = expr_len {
                    quote!(::#KRATE::trailing::Trailing::create_fut_from_len(#size_expr, reader, #options_transition))
                } else if let Some(payload_len) = payload_len {
                    let len = payload_len.as_parsed_length();

                    quote!(::#KRATE::trailing::Trailing::create_fut_from_len(#len, reader, #options_transition))
                } else {
                    quote!(::#KRATE::trailing::Trailing::create_fut(reader, #options_transition))
                }
            }
            Self::Payload(expr_len) => {
                if let Some(LenFromExpr { size_expr, .. }) = expr_len {
                    quote!(::#KRATE::payload::Payload::create_fut_from_len(#size_expr, reader, #options_transition))
                } else if let Some(payload_len) = payload_len {
                    let len = payload_len.as_parsed_length();

                    quote!(::#KRATE::payload::Payload::create_fut_from_len(#len, reader, #options_transition))
                } else {
                    quote!(::#KRATE::payload::Payload::create_fut(reader, #options_transition))
                }
            }
            Self::Padded(expr_len) => {
                if let Some(LenFromExpr { size_expr, .. }) = expr_len {
                    quote!(::#KRATE::padded::Padded::create_fut_from_len(#size_expr, reader, #options_transition))
                } else if let Some(payload_len) = payload_len {
                    let len = payload_len.as_parsed_length();

                    quote!(::#KRATE::padded::Padded::create_fut_from_len(#len, reader, #options_transition))
                } else {
                    unreachable!()
                }
            }
            Self::Tagged(expr_len, children, tagged_name) => {
                let tag_ty = &tag.expect("no tag for given tagged field").ty;

                let reader = reader_from_lengths(*expr_len, payload_len);

                let async_name = quote::format_ident!("Async{}Parse", tagged_name);
                let parse_name = if expr_len.is_some() || payload_len.is_some() {
                    quote::format_ident!("{async_name}WithLen")
                } else {
                    async_name
                };

                let tag_bindings = ChildList::bind_overridden_tag_consts(&children, None, tag_ty);
                let async_parse =
                    ChildList::async_parse(children, None, tag_ty, mod_name, &parse_name, false);
                quote! {
                    {
                        let reader = #reader;

                        #(#tag_bindings)*

                        match _tag {
                            #(#async_parse)*
                            missed => {
                                let extent = ::#KRATE::reader::PollReader::extent(&reader);
                                let rest = ::#KRATE::reader::PollReader::remaining_size(&reader);
                                #mod_name #parse_name ::Seek(#mod_name #tagged_name::Unsupported(::#KRATE::children::UnknownChild::new(missed, extent)), ::#KRATE::reader::AsyncSeek::seek(reader, rest), opts)
                            }
                        }
                    }
                }
            }
            FieldKind::Conditional(cond) => {
                let enum_name = match cond.group().unwrap() {
                    GroupedConditionals::Enum { name, .. } => {
                        quote::format_ident!("Async{name}Parse")
                    }
                    GroupedConditionals::Struct { name, .. } => {
                        quote::format_ident!("AsyncOptional{name}Parse")
                    }
                };

                cond.format_conditional(
                    |name, fields, exhaustive| {
                        let payload_len = AtomFields::nested_payload_len(payload_len, fields);
                        let tag = AtomFields::nested_tag(tag, fields);
                        let typed = prepare_fields_for_statemachine(
                            fields,
                            generics.clone(),
                            mod_name,
                            payload_len.as_ref(),
                        );
                        let parse_name = quote::format_ident!("Async{}Parse", name);
                        if let Some((nested_next_name, nested_next_ty, nested_next_kind, nested_next_endianess)) =
                            typed.iter().find_map(|f| f.as_ref())
                        {
                            let fut = nested_next_kind.as_next_fut(
                                nested_next_ty,
                                generics,
                                mod_name,
                                generic_collection,
                                payload_len.as_ref(),
                                tag.as_ref(),
                                current_field_endianess,
                                *nested_next_endianess,
                                definition_endianess,
                                false,
                            );

                            let name = if !exhaustive {
                                quote::format_ident!("Some{}", name)
                            } else {
                                name.clone()
                            };

                            let starting_idx = typed.iter().position(|f| f.is_some()).unwrap();
                            let starting_state = quote::format_ident!("S{}", starting_idx);

                            quote! {
                                #mod_name #enum_name:: #name (#mod_name #parse_name::#starting_state {
                                    #nested_next_name: #fut,
                                })
                            }
                        } else {
                            let name = if !exhaustive {
                                quote::format_ident!("Some{}", name)
                            } else {
                                name.clone()
                            };

                            quote! {
                                #mod_name #enum_name:: #name (#mod_name #parse_name::S0 {
                                    nop: (reader, opts, ::core::marker::PhantomData)
                                })
                            }
                        }
                    },
                    || {
                        quote! {
                            #mod_name #enum_name:: None (reader, opts, ::core::marker::PhantomData)
                        }
                    },
                )
            }
            _ => {
                quote!(<#next_ty as ::#KRATE::parse::AsyncParse<__Offset>>::create_fut(reader, #options_transition))
            }
        }
    }
}

pub fn prepare_fields_for_statemachine<'a>(
    fields: &'a [AtomField],
    mut generics: syn::Generics,
    mod_name: Option<&proc_macro2::TokenStream>,
    payload_len: Option<&PayloadLength>,
) -> Vec<
    Option<(
        syn::Ident,
        proc_macro2::TokenStream,
        FieldKind<'a>,
        Option<&'a Endianess>,
    )>,
> {
    let where_clause = generics
        .where_clause
        .get_or_insert_with(|| syn::WhereClause {
            where_token: <syn::Token![where]>::default(),
            predicates: syn::punctuated::Punctuated::new(),
        });

    for param in &generics.params {
        if let syn::GenericParam::Type(ty) = param {
            let name = &ty.ident;
            where_clause.predicates.push(syn::parse_quote!(<#name as ::#KRATE::parse::Parsed>::Output<__Offset>: ::core::marker::Unpin));
        }
    }

    use quote::quote;

    fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let (name, ty, kind) = match &field.kind {
                AtomFieldKind::FullBox { name, .. } => {
                    let ty = quote!(#mod_name #name);
                    let name = syn::Ident::new("atom_flags", proc_macro2::Span::call_site());
                    (name, ty, FieldKind::Regular)
                }
                AtomFieldKind::Flags {
                    field_name, name, ..
                } => {
                    let ty = quote!(#mod_name #name);
                    let name = field_name
                        .clone()
                        .unwrap_or(syn::Ident::new("flags", proc_macro2::Span::call_site()));
                    (name, ty, FieldKind::Regular)
                }
                AtomFieldKind::Version { .. } => unreachable!(),
                AtomFieldKind::Children { size_ty, kind, children } => {
                    match kind {
                        ChildrenKind::Atom => {
                            let name = syn::Ident::new("children", proc_macro2::Span::call_site());

                            let ty = if let Some(size_ty) = size_ty {
                                quote!(::#KRATE::children::SizedChildren<__Offset, #size_ty, #mod_name Child>)
                            } else {
                                quote!(::#KRATE::children::Children<__Offset, #mod_name Child>)
                            };
                            (name, ty, FieldKind::Regular)
                        }
                        ChildrenKind::Tagged { field_name, tagged_name, len_from_expr, .. } => {
                            (field_name.clone(), quote!(#mod_name #tagged_name), FieldKind::Tagged(len_from_expr.as_ref(), children, tagged_name))
                        }
                    }
                }
                AtomFieldKind::Conditional(cond) => {
                    let name = cond.field_name.clone();
                    (name, quote!(ConditionalStorage), FieldKind::Conditional(cond))
                }
                AtomFieldKind::Struct(field, attr) => match attr {
                    StructFieldAttr::Normal => {
                        let name = field.ident.clone().unwrap();
                        let ty = &field.ty;

                        (name, quote!(#ty), FieldKind::Regular)
                    }
                    StructFieldAttr::VersionIdentifier => unreachable!(),
                    StructFieldAttr::Tag => {
                        let name = quote::format_ident!("_tag");
                        let ty = &field.ty;

                        (name, quote!(#ty), FieldKind::Tag)
                    }
                    StructFieldAttr::PayloadLength{ .. } => {
                        let name = quote::format_ident!("_payload_len");
                        let ty = &field.ty;

                        (name, quote!(#ty), FieldKind::PayloadLength)
                    }
                    StructFieldAttr::Payload { from_expr } => {
                        let name = field.ident.clone().unwrap();
                        let display_ty = if let Some(LenFromExpr { ty: len_ty, .. }) = from_expr {
                            quote!(::#KRATE::payload::Payload<__Offset, #len_ty>)
                        } else if let Some(PayloadLength { ty: len_ty,  ..}) = payload_len {
                            quote!(::#KRATE::payload::Payload<__Offset, #len_ty>)
                        } else {
                            quote!(::#KRATE::payload::Payload)
                        };
                        (name, display_ty, FieldKind::Payload(from_expr.as_ref()))
                    }
                    StructFieldAttr::Padded { from_expr } => {
                        let name = field.ident.clone().unwrap();
                        let ty = &field.ty;
                        assert!(from_expr.is_some() || payload_len.is_some(), "padded field requires either a length specifier or a payload_len field");
                        (name, quote!(#ty), FieldKind::Padded(from_expr.as_ref()))
                    }
                    StructFieldAttr::PascalString { length_ty } => {
                        let name = field.ident.clone().unwrap();
                        (
                            name,
                            quote!(::#KRATE::string::PascalString<__Offset, #length_ty>),
                            FieldKind::Regular,
                        )
                    }
                    StructFieldAttr::NullTerminatedString => {
                        let name = field.ident.clone().unwrap();
                        (
                            name,
                            quote!(::#KRATE::string::NullTerminatedString<__Offset>),
                            FieldKind::Regular,
                        )
                    }
                    StructFieldAttr::Reserved => {
                        let name = quote::format_ident!("__reserved_{i}");
                        let ty = &field.ty;
                        (name, quote!(#ty), FieldKind::Reserved)
                    }
                    StructFieldAttr::Expected { val } => {
                        let name = quote::format_ident!("__expected_{i}");
                        let ty = &field.ty;
                        (name, quote!(#ty), FieldKind::Expected(val))
                    }
                    StructFieldAttr::DynamicArray {
                        length_ty,
                        zero_relative,
                    } => {
                        let ty = &field.ty;
                        let name = field.ident.clone().unwrap();
                        (
                            name,
                            quote!(::#KRATE::array::DynamicArray<__Offset, #length_ty, #ty, #zero_relative>),
                            FieldKind::Regular,
                        )
                    }
                    StructFieldAttr::NullTerminatedArray => {
                        let ty = &field.ty;
                        let name = field.ident.clone().unwrap();
                        (
                            name,
                            quote!(::#KRATE::array::NullTerminated<__Offset, #ty>),
                            FieldKind::Regular,
                        )
                    }
                    StructFieldAttr::TrailingArray { from_expr } => {
                        let ty = &field.ty;
                        let name = field.ident.clone().unwrap();
                        let display_ty = if let Some(LenFromExpr { ty: len_ty, .. }) = from_expr {
                            quote!(::#KRATE::trailing::Trailing<__Offset, #ty, #len_ty>)
                        } else if let Some(PayloadLength { ty: len_ty, ..}) = payload_len {
                            quote!(::#KRATE::trailing::Trailing<__Offset, #ty, #len_ty>)
                        } else {
                            quote!(::#KRATE::trailing::Trailing<__Offset, #ty>)
                        };
                        (name, display_ty, FieldKind::Trailing(from_expr.as_ref()))
                    }
                    StructFieldAttr::Condition => {
                        let name = field.ident.clone().unwrap();
                        let ty = &field.ty;

                        (name, quote!(#ty), FieldKind::Condition)
                    }
                    //ignore moving fields around until after parsing is finished
                    StructFieldAttr::ReuseCondition => return None,
                }
            };
            Some((name, ty, kind, field.endianess.as_ref()))
        })
        .collect::<Vec<_>>()
}

pub fn format_statemachine_nop_impl(
    typed_fields: &[Option<(
        syn::Ident,
        proc_macro2::TokenStream,
        FieldKind,
        Option<&Endianess>,
    )>],
    generics: &syn::Generics,
    mod_name: Option<&proc_macro2::TokenStream>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    payload_len: Option<&PayloadLength>,
    tag: Option<&Tag>,
    definition_endianess: Option<&Endianess>,
    format_state_transition: &dyn Fn(
        proc_macro2::TokenStream,
        Option<Endianess>,
    ) -> proc_macro2::TokenStream,
) -> (
    proc_macro2::TokenStream,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
) {
    use quote::quote;
    if let Some((name, ty, kind, endianess)) = typed_fields.iter().find_map(|f| f.as_ref()) {
        let next_fut = kind.as_next_fut(
            ty,
            generics,
            mod_name,
            generic_collection,
            payload_len,
            tag,
            None,
            *endianess,
            definition_endianess,
            true,
        );

        (
            quote! {
                #name: #next_fut
            },
            None,
            None,
            None,
        )
    } else {
        let state_transition = format_state_transition(Default::default(), None);

        let pd_generics = generics.params.iter().filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(&t.ident),
            _ => None,
        });
        let format_options =
            Endianess::format_options_create_fut(definition_endianess, None, false);
        let options_ty = Endianess::format_options_ty(definition_endianess, None);
        let take_opts = Endianess::format_take_reader(definition_endianess, None);
        let borrow_opts = Endianess::format_borrow_reader(definition_endianess, None);

        (
            quote! {
                nop: (reader, #format_options, ::core::marker::PhantomData)
            },
            Some(quote! {
                S0 { nop: (__Reader, #options_ty, ::core::marker::PhantomData<( __Offset, #(#pd_generics,)* )>) },
            }),
            Some(quote! {
                Self::S0 { nop: (reader, opts, _) } => {
                    let opts = #take_opts;
                    #state_transition
                }
            }),
            Some(quote! {
                Self::S0 { nop: (reader, opts, _) } => (reader, #borrow_opts),
            }),
        )
    }
}

pub fn format_variant_state_change<'a>(
    name: &syn::Ident,
    variant_name: &syn::Ident,
    enum_name: &syn::Ident,
    variant_parse_name: &syn::Ident,
    fields: &[AtomField],
    typed_fields: &[Option<(
        syn::Ident,
        proc_macro2::TokenStream,
        FieldKind,
        Option<&Endianess>,
    )>],
    exhaustive: bool,
    generics: &syn::Generics,
    match_fields: impl Iterator<Item = &'a syn::Ident> + Clone,
    mod_name: Option<&proc_macro2::TokenStream>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    root: bool,
    payload_len: Option<&PayloadLength>,
    tag: Option<&Tag>,
    struct_name: &syn::Ident,
    idx: usize,
    parse_name: &syn::Ident,
    state: &syn::Ident,
    on_poll_ready: &proc_macro2::TokenStream,
    definition_endianess: Option<&Endianess>,
    format_conditional_state: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    format_state_transition: &dyn Fn(
        proc_macro2::TokenStream,
        Option<Endianess>,
    ) -> proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    use quote::quote;
    let variant = variant_name;

    let parent_typed_fields = &typed_fields;

    let payload_len = AtomFields::nested_payload_len(payload_len, fields);
    let tag = AtomFields::nested_tag(tag, fields);

    let typed_fields =
        prepare_fields_for_statemachine(fields, generics.clone(), mod_name, payload_len.as_ref());

    let format_nested_conditional = |toks| {
        let match_fields = match_fields.clone();
        let variant = if !exhaustive {
            quote::format_ident!("Some{}", variant)
        } else {
            variant.clone()
        };

        let enum_parse_name = quote::format_ident!("Async{}Parse", enum_name);

        let parse_mod = if root { None } else { mod_name };

        let toks = quote! {
            #parse_mod #parse_name::#state {
                #(#match_fields,)*
                #name: #mod_name #enum_parse_name::#variant(#toks),
            }
        };
        format_conditional_state(toks)
    };

    let format_state_transition = |toks, prev_endianess: Option<Endianess>| {
        let finished = typed_fields.iter().filter_map(|f| {
            let (f, _, kind, _) = f.as_ref()?;
            match kind {
                FieldKind::PayloadLength
                | FieldKind::Tag
                | FieldKind::Reserved
                | FieldKind::Expected(_)
                | FieldKind::Condition => None,
                _ => Some(f),
            }
        });

        let parse_variant = if !exhaustive {
            quote::format_ident!("Some{}", variant)
        } else {
            variant.clone()
        };

        let enum_storage_name = quote::format_ident!("Async{}Storage", enum_name);
        let variant_storage_name = quote::format_ident!("Async{}Storage", variant);

        let mut generics = generics.clone();
        generics.params.push(syn::parse_quote!(__Offset));
        let (_, ty_generics, _) = generics.split_for_impl();

        let var = quote! {
            // any finished nested state will be here
            #toks

            let #name = #mod_name #enum_storage_name::#ty_generics::#parse_variant(#mod_name #variant_storage_name {
                #(#finished,)*
                #generic_collection
                _pd: ::core::marker::PhantomData,
            });
        };

        if let Some(next_pos) = parent_typed_fields[(idx + 1)..]
            .iter()
            .position(|f| f.is_some())
        {
            let (_, next_fut, state) = format_statemachine_next_state(
                parent_typed_fields,
                idx + 1 + next_pos,
                payload_len.as_ref(),
                tag.as_ref(),
                parse_name,
                mod_name,
                generic_collection,
                prev_endianess.as_ref(),
                definition_endianess,
                format_conditional_state,
                &generics,
                root,
            );
            quote! {
                #var
                #next_fut
                *self = #state;
            }
        } else {
            format_state_transition(var, prev_endianess)
        }
    };

    let parse_name = quote::format_ident!("Async{}Parse", variant);
    let (_state_machine_variants, state_machine_variant_parsing, _borrow_reader_variants) =
        format_statemachine_fields(
            fields,
            &typed_fields,
            payload_len.as_ref(),
            tag.as_ref(),
            struct_name,
            &parse_name,
            &generics,
            generic_collection,
            mod_name,
            definition_endianess,
            &format_nested_conditional,
            &format_state_transition,
            false,
        );

    let vname = &variant;
    let variant = if !exhaustive {
        quote::format_ident!("Some{}", variant)
    } else {
        variant.clone()
    };

    let nop_variant = if typed_fields.iter().all(|f| f.is_none()) {
        let storage_name = quote::format_ident!("Async{}Storage", enum_name);
        let variant_storage_name = quote::format_ident!("Async{}Storage", vname);

        let mut generics = generics.clone();
        generics.params.push(syn::parse_quote!(__Offset));
        let (_, ty_generics, _) = generics.split_for_impl();

        Some(quote! {
            #mod_name #parse_name ::S0 { nop: (reader, opts, _) } => {
                let #name = #mod_name #storage_name:: #ty_generics ::#variant(#mod_name #variant_storage_name { _pd: ::core::marker::PhantomData });
                #on_poll_ready
            }
        })
    } else {
        None
    };

    quote! {
        #mod_name #variant_parse_name::#variant(v) => {
            match v {
                #(#state_machine_variant_parsing)*
                #nop_variant
            }
        }
    }
}

fn format_none_variant_state_change(
    exhaustive: bool,
    enum_name: &syn::Ident,
    mod_name: Option<&proc_macro2::TokenStream>,
    variant_parse_name: &syn::Ident,
    name: &syn::Ident,
    on_poll_ready: &proc_macro2::TokenStream,
    ty_generics: &syn::TypeGenerics,
) -> Option<proc_macro2::TokenStream> {
    if !exhaustive {
        let storage_name = quote::format_ident!("Async{}Storage", enum_name);

        Some(
            quote::quote! {
                #mod_name #variant_parse_name::None(reader, opts, _) => {
                    let #name = #mod_name #storage_name::#ty_generics::None;
                    #on_poll_ready
                }
            }
            .into(),
        )
    } else {
        None
    }
}

pub fn format_statemachine_fields(
    fields: &[AtomField],
    typed_fields: &[Option<(
        syn::Ident,
        proc_macro2::TokenStream,
        FieldKind,
        Option<&Endianess>,
    )>],
    payload_len: Option<&PayloadLength>,
    tag: Option<&Tag>,
    struct_name: &syn::Ident,
    parse_name: &syn::Ident,
    generics: &syn::Generics,
    generic_collection: Option<&proc_macro2::TokenStream>,
    mod_name: Option<&proc_macro2::TokenStream>,
    definition_endianess: Option<&Endianess>,
    format_conditional_state: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    format_state_transition: &dyn Fn(
        proc_macro2::TokenStream,
        Option<Endianess>,
    ) -> proc_macro2::TokenStream,
    root: bool,
) -> (
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    use quote::quote;
    let mut state_machine_variants = vec![];
    let mut state_machine_variant_parsing = vec![];
    let mut borrow_reader_variants = vec![];

    let payload_len = AtomFields::nested_payload_len(payload_len, fields);
    let tag = AtomFields::nested_tag(tag, fields);

    for idx in 0..typed_fields.len() {
        let completed = &typed_fields[0..idx];
        let Some(in_progress) = &typed_fields[idx] else {
            continue;
        };
        let state = quote::format_ident!("S{}", idx);

        let variant_fields = completed.iter().filter_map(|f| {
            let (name, ty, kind, _) = f.as_ref()?;
            match kind {
                FieldKind::Reserved | FieldKind::Expected { .. } => None,
                FieldKind::Conditional(cond) => {
                    let storage_name = match cond.group().unwrap() {
                        GroupedConditionals::Struct { name, .. } => {
                            quote::format_ident!("AsyncOptional{}Storage", name)
                        }
                        GroupedConditionals::Enum { name, .. } => {
                            quote::format_ident!("Async{}Storage", name)
                        }
                    };

                    let mut storage_generics = generics.clone();
                    storage_generics.params.push(syn::parse_quote!(__Offset));
                    let (_, storage_ty_generics, _) = storage_generics.split_for_impl();

                    Some(quote! {
                        #name: #mod_name #storage_name #storage_ty_generics,
                    })
                }
                _ => Some(quote! {
                    #name: <#ty as ::#KRATE::parse::Parsed>::Output<__Offset>,
                }),
            }
        });

        let in_progress_fut = {
            let (name, ty, kind, endianess) = in_progress;
            let options_ty = Endianess::format_options_ty(
                *endianess,
                Some(Endianess::format_options_ty(definition_endianess, None)),
            );

            match kind {
                FieldKind::Payload(expr_len) => {
                    if let Some(LenFromExpr { ty: len_ty, .. }) = expr_len {
                        quote!(#name: ::#KRATE::payload::PayloadParse<#options_ty, __Offset, __Reader, #len_ty>)
                    } else if let Some(PayloadLength { ty: len, .. }) = payload_len.as_ref() {
                        quote!(#name: ::#KRATE::payload::PayloadParse<#options_ty, __Offset, __Reader, #len>)
                    } else {
                        quote!(#name: ::#KRATE::payload::PayloadParse<#options_ty, __Offset, __Reader>)
                    }
                }
                FieldKind::Trailing(expr_len) => {
                    let AtomFieldKind::Struct(field, _) = &fields[idx].kind else {
                        unreachable!();
                    };
                    let trailing_ty = &field.ty;
                    if let Some(LenFromExpr { ty: len_ty, .. }) = expr_len {
                        quote!(#name: ::#KRATE::trailing::TrailingParse<#options_ty, __Offset, __Reader, #trailing_ty, #len_ty>)
                    } else if let Some(PayloadLength { ty: len, .. }) = payload_len.as_ref() {
                        quote!(#name: ::#KRATE::trailing::TrailingParse<#options_ty, __Offset, __Reader, #trailing_ty, #len>)
                    } else {
                        quote!(#name: ::#KRATE::trailing::TrailingParse<#options_ty, __Offset, __Reader, #trailing_ty>)
                    }
                }
                FieldKind::Padded(expr_len) => {
                    let len_ty = match (expr_len, payload_len.as_ref()) {
                        (Some(LenFromExpr { ty: len_ty, .. }), _) => len_ty,
                        (_, Some(PayloadLength { ty: len_ty, .. })) => len_ty,
                        _ => unreachable!(),
                    };
                    quote!(#name: ::#KRATE::padded::AsyncPaddedParse<#options_ty, __Offset, __Reader, #len_ty, #ty>)
                }
                FieldKind::Tagged(expr_len, _, tagged_name) => {
                    let async_name = quote::format_ident!("Async{tagged_name}Parse");
                    if expr_len.is_some() || payload_len.is_some() {
                        let with_len = quote::format_ident!("{async_name}WithLen");
                        quote!(#name: #mod_name #with_len<#options_ty, __Offset, ::#KRATE::reader::TrailingReader<__Offset, __Reader>>,)
                    } else {
                        quote!(#name: #mod_name #async_name<#options_ty, __Offset, __Reader>,)
                    }
                }
                FieldKind::Conditional(cond) => {
                    let field_name = &cond.field_name;
                    let parse_name = match cond.group().unwrap() {
                        GroupedConditionals::Struct { name, .. } => {
                            quote::format_ident!("AsyncOptional{}Parse", name)
                        }
                        GroupedConditionals::Enum { name, .. } => {
                            quote::format_ident!("Async{}Parse", name)
                        }
                    };

                    let mut parse_generics = generics.clone();
                    parse_generics
                        .params
                        .push(syn::parse_quote!(__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin));
                    parse_generics.params.push(syn::parse_quote!(__Offset));
                    parse_generics.params.push(syn::parse_quote!(__Reader));
                    let (_, parse_ty_generics, _) = parse_generics.split_for_impl();

                    quote!(#field_name: #mod_name #parse_name #parse_ty_generics)
                }
                _ => {
                    quote! {
                        #name: <#ty as ::#KRATE::parse::AsyncParse<__Offset>>::Fut<#options_ty, __Reader>,
                    }
                }
            }
        };

        let variant = quote! {
            #state {
                #(#variant_fields)*
                #in_progress_fut
            }
        };

        let name = &in_progress.0;

        let (def_endianess, in_progress_endianess) =
            if matches!(in_progress.2, FieldKind::Conditional(_)) {
                (None, None)
            } else {
                (definition_endianess, in_progress.3)
            };

        let borrow_opts = Endianess::format_borrow_reader(in_progress_endianess, def_endianess);
        let borrow_reader = quote! {
            Self::#state {
                #name, ..
            } => {
                let (reader, opts) = #name.borrow_reader();
                (reader, #borrow_opts)
            }
        };

        let on_poll_ready =
            if let Some(next_pos) = typed_fields[(idx + 1)..].iter().position(|f| f.is_some()) {
                let (_, next_fut, state) = format_statemachine_next_state(
                    typed_fields,
                    idx + 1 + next_pos,
                    payload_len.as_ref(),
                    tag.as_ref(),
                    parse_name,
                    mod_name,
                    generic_collection,
                    in_progress.3,
                    definition_endianess,
                    format_conditional_state,
                    &generics,
                    root,
                );
                quote! {
                    #next_fut
                    *self = #state;
                }
            } else {
                format_state_transition(Default::default(), in_progress.3.copied())
            };

        let check_reserved = match in_progress.2 {
            FieldKind::Reserved => {
                let ty = &in_progress.1;
                let take_opts = Endianess::format_take_reader(in_progress.3, definition_endianess);
                Some(quote! {
                    if ::#KRATE::parse_options::Options::options(&opts).error_on_used_reserved_fields && (#name != unsafe { ::core::mem::zeroed::<#ty>() }) {
                        *self = Self::Done(reader, #take_opts);
                        return ::core::task::Poll::Ready(Err(::#KRATE::error::ParseError::UsedReservedField));
                    }
                })
            }
            FieldKind::Expected(val) => {
                let take_opts = Endianess::format_take_reader(in_progress.3, definition_endianess);
                Some(quote! {
                    if #name != #val {
                        *self = Self::Done(reader, #take_opts);
                        return ::core::task::Poll::Ready(Err(::#KRATE::error::ParseError::UnexpectedValue));
                    }
                })
            }
            _ => None,
        };

        let match_fields = completed.iter().filter_map(|f| {
            let (name, _, kind, _) = f.as_ref()?;
            if matches!(kind, FieldKind::Reserved | FieldKind::Expected(_)) {
                return None;
            }

            Some(name)
        });

        let on_state = if let FieldKind::Conditional(cond) = in_progress.2 {
            let exhaustive = cond.is_exhaustive();

            let sm_parse = match cond.group().unwrap() {
                GroupedConditionals::Struct {
                    name: enum_name,
                    fields,
                } => {
                    let variant_name = &enum_name;
                    let enum_name = quote::format_ident!("Optional{}", enum_name);
                    let variant_parse_name = quote::format_ident!("Async{}Parse", enum_name);

                    let variant = format_variant_state_change(
                        name,
                        variant_name,
                        &enum_name,
                        &variant_parse_name,
                        &fields,
                        &typed_fields,
                        exhaustive,
                        generics,
                        match_fields.clone(),
                        mod_name,
                        generic_collection,
                        root,
                        payload_len.as_ref(),
                        tag.as_ref(),
                        struct_name,
                        idx,
                        parse_name,
                        &state,
                        &on_poll_ready,
                        definition_endianess,
                        format_conditional_state,
                        format_state_transition,
                    );

                    let mut generics = generics.clone();
                    generics.params.push(syn::parse_quote!(__Offset));
                    let (_, ty_generics, _) = generics.split_for_impl();
                    let none_variant = format_none_variant_state_change(
                        exhaustive,
                        &enum_name,
                        mod_name,
                        &variant_parse_name,
                        name,
                        &on_poll_ready,
                        &ty_generics,
                    );

                    quote! {
                        match #name {
                            #variant
                            #none_variant
                        }
                    }
                }
                GroupedConditionals::Enum {
                    name: enum_name,
                    variants,
                } => {
                    let variant_parse_name = quote::format_ident!("Async{}Parse", enum_name);

                    let vs = variants.iter().map(|v| {
                        format_variant_state_change(
                            name,
                            &v.variant_name,
                            &enum_name,
                            &variant_parse_name,
                            &v.fields,
                            &typed_fields,
                            exhaustive,
                            generics,
                            match_fields.clone(),
                            mod_name,
                            generic_collection,
                            root,
                            payload_len.as_ref(),
                            tag.as_ref(),
                            struct_name,
                            idx,
                            parse_name,
                            &state,
                            &on_poll_ready,
                            definition_endianess,
                            format_conditional_state,
                            format_state_transition,
                        )
                    });

                    let mut generics = generics.clone();
                    generics.params.push(syn::parse_quote!(__Offset));
                    let (_, ty_generics, _) = generics.split_for_impl();
                    let none_variant = format_none_variant_state_change(
                        exhaustive,
                        &enum_name,
                        mod_name,
                        &variant_parse_name,
                        name,
                        &on_poll_ready,
                        &ty_generics,
                    );

                    quote! {
                        match #name {
                            #(#vs)*
                            #none_variant
                        }
                    }
                }
            };

            let mod_name = if root { None } else { mod_name };

            quote! {
                #mod_name #parse_name::#state {
                    #(#match_fields,)*
                    mut #name,
                } => {
                    #sm_parse
                }
            }
        } else {
            let pending = {
                let match_fields = match_fields.clone();
                let mod_name = if root { None } else { mod_name };
                quote! {
                    #mod_name #parse_name::#state {
                        #(#match_fields,)*
                        #name,
                    }
                }
            };
            let pending = format_conditional_state(pending);

            let mod_name = if root { None } else { mod_name };

            let take_opts = Endianess::format_take_reader(in_progress.3, definition_endianess);

            quote! {
                #mod_name #parse_name::#state {
                    #(#match_fields,)*
                    mut #name,
                } => {
                    match ::core::pin::Pin::new(&mut #name).poll(cx) {
                        ::core::task::Poll::Pending => {
                            *self = #pending;
                            return ::core::task::Poll::Pending;
                        }
                        ::core::task::Poll::Ready(r) => {
                            let (reader, opts) = ::#KRATE::reader::TakeReader::take_reader(#name);
                            let #name = match r {
                                Ok(#name) => #name,
                                Err(e) => {
                                    *self = Self::Done(reader, #take_opts);
                                    return ::core::task::Poll::Ready(Err(e));
                                }
                            };
                            #check_reserved
                            #on_poll_ready
                        }
                    }
                }
            }
        };

        state_machine_variants.push(variant);
        state_machine_variant_parsing.push(on_state);
        borrow_reader_variants.push(borrow_reader);
    }
    (
        state_machine_variants,
        state_machine_variant_parsing,
        borrow_reader_variants,
    )
}

pub fn format_statemachine_next_state(
    typed_fields: &[Option<(
        syn::Ident,
        proc_macro2::TokenStream,
        FieldKind,
        Option<&Endianess>,
    )>],
    idx: usize,
    payload_len: Option<&PayloadLength>,
    tag: Option<&Tag>,
    parse_name: &syn::Ident,
    mod_name: Option<&proc_macro2::TokenStream>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    current_field_endianess: Option<&Endianess>,
    definition_endianess: Option<&Endianess>,
    format_conditional_state: &dyn Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    generics: &syn::Generics,
    root: bool,
) -> (
    syn::Ident,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    use quote::quote;

    let (next_name, next_ty, next_kind, next_endianess) = typed_fields[idx].as_ref().unwrap();
    let next_state = quote::format_ident!("S{}", idx);
    let new_completed = typed_fields[..idx].iter().filter_map(|f| {
        let (name, _, kind, _) = f.as_ref()?;
        if matches!(kind, FieldKind::Reserved | FieldKind::Expected(_)) {
            return None;
        }
        Some(name)
    });

    let next_fut = next_kind.as_next_fut(
        next_ty,
        generics,
        mod_name,
        generic_collection,
        payload_len,
        tag,
        current_field_endianess,
        *next_endianess,
        definition_endianess,
        false,
    );
    let next_fut = quote! {
        let #next_name = #next_fut;
    };

    let mod_name = if root { None } else { mod_name };

    let state = quote! {
        #mod_name #parse_name::#next_state {
            #(#new_completed,)*
            #next_name,
            #generic_collection
        }
    };
    // inlines the conditional state
    // e.g
    // let is2 = <create_future>;
    // OuterParse::S3 {
    //   os1,
    //   os2,
    //   s3: InnerParse::S2 {
    //      is1,
    //      is2,
    //   },
    // }
    let state = format_conditional_state(state);
    (next_state, next_fut, state)
}

pub fn format_async_statemachine(
    fields: &[AtomField],
    struct_name: &syn::Ident,
    mut generics: syn::Generics,
    atom_mod: Option<&syn::Ident>,
    generic_collection: Option<&proc_macro2::TokenStream>,
    payload_len: Option<&PayloadLength>,
    tag: Option<&Tag>,
    definition_endianess: Option<&Endianess>,
) -> proc_macro2::TokenStream {
    let payload_len = AtomFields::nested_payload_len(payload_len, fields);
    let tag = AtomFields::nested_tag(tag, fields);
    let mod_name = atom_mod.map(|name| quote!(#name::));
    let typed_fields = prepare_fields_for_statemachine(
        fields,
        generics.clone(),
        mod_name.as_ref(),
        payload_len.as_ref(),
    );
    use quote::quote;

    let parse_name = quote::format_ident!("Async{}Parse", struct_name);

    let format_base = |toks| toks;
    let format_state_transition = |toks, prev_endianess: Option<Endianess>| {
        let finished = fields.iter().filter_map(|f| match &f.kind {
            AtomFieldKind::Conditional(cond) => {
                Some(cond.as_async_field_collection(mod_name.as_ref()))
            }
            _ => f.as_collection(),
        });

        let def_endianess = if fields.is_empty() {
            None
        } else {
            definition_endianess
        };

        let opts_transition = Endianess::format_take_reader(def_endianess, prev_endianess.as_ref());
        quote! {
            #toks

            *self = Self::Done(reader, #opts_transition);
            return ::core::task::Poll::Ready(Ok(#struct_name {
                #(#finished)*
                #generic_collection
                _offset: ::core::marker::PhantomData,
            }));
        }
    };

    let (state_machine_variants, state_machine_variant_parsing, borrow_reader_variants) =
        format_statemachine_fields(
            fields,
            &typed_fields,
            payload_len.as_ref(),
            tag.as_ref(),
            struct_name,
            &parse_name,
            &generics,
            generic_collection,
            mod_name.as_ref(),
            definition_endianess,
            &format_base,
            &format_state_transition,
            true,
        );

    let (start, nop_variant, nop_poll, nop_borrow) = format_statemachine_nop_impl(
        &typed_fields,
        &generics,
        mod_name.as_ref(),
        generic_collection,
        payload_len.as_ref(),
        tag.as_ref(),
        definition_endianess,
        &format_state_transition,
    );

    let where_clause = generics
        .where_clause
        .get_or_insert_with(|| syn::WhereClause {
            where_token: <syn::Token![where]>::default(),
            predicates: syn::punctuated::Punctuated::new(),
        });

    for param in &generics.params {
        if let syn::GenericParam::Type(ty) = param {
            let name = &ty.ident;
            where_clause.predicates.push(syn::parse_quote!(<#name as ::#KRATE::parse::Parsed>::Output<__Offset>: ::core::marker::Unpin));
        }
    }

    for param in &mut generics.params {
        if let syn::GenericParam::Type(ty) = param {
            ty.bounds.push(syn::parse_quote!(::core::marker::Unpin));
            ty.bounds
                .push(syn::parse_quote!(::#KRATE::parse::AsyncParse<__Offset>));
        }
    }

    let mut parse_generics = generics.clone();
    parse_generics
        .params
        .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin));
    let (parse_impl_generics, _, _) = parse_generics.split_for_impl();

    let struct_generics = generics.clone();
    let (_, struct_ty_generics, struct_where_clause) = struct_generics.split_for_impl();

    generics.params.push(
        syn::parse_quote!(__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin),
    );
    generics
        .params
        .push(syn::parse_quote!(__Offset: ::#KRATE::reader::Offset + ::core::marker::Unpin));
    generics.params.push(
        syn::parse_quote!(__Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin),
    );

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let starting_idx = typed_fields.iter().position(|f| f.is_some()).unwrap_or(0);
    let starting_state = quote::format_ident!("S{}", starting_idx);

    quote! {
        pub enum #parse_name #generics #where_clause {
            #(#state_machine_variants,)*
            #nop_variant
            Done(__Reader, __Options),
            Empty,
        }

        impl #impl_generics Future for #parse_name #type_generics #where_clause {
            type Output = ::core::result::Result<<#struct_name #struct_ty_generics as ::#KRATE::parse::Parsed>::Output<__Offset>, ::#KRATE::error::ParseError>;
            fn poll(mut self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                loop {
                    let mut this = ::core::mem::replace(&mut *self, Self::Empty);
                    match this {
                        #(#state_machine_variant_parsing)*
                        #nop_poll
                        Self::Done(..) => panic!("future polled after completion"),
                        Self::Empty => unreachable!(),
                    }
                }
            }
        }

        impl #parse_impl_generics ::#KRATE::parse::AsyncParse<__Offset> for #struct_name #struct_ty_generics #struct_where_clause {
            type Fut<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin> = #parse_name #type_generics ;
            fn create_fut<__Options: ::#KRATE::parse_options::Options + ::core::marker::Unpin, __Reader: ::#KRATE::reader::PollReader<__Offset> + ::core::marker::Unpin>(reader: __Reader, opts: __Options) -> Self::Fut<__Options, __Reader> {
                #parse_name :: #starting_state {
                    #start
                }
            }
        }

        impl #impl_generics ::#KRATE::reader::TakeReader<__Options, __Reader> for #parse_name #type_generics #where_clause {
            fn take_reader(self) -> (__Reader, __Options) {
                match self {
                    Self::Done(reader, opts) => return (reader, opts),
                    _ => unreachable!("invalid state"),
                }
            }

            fn borrow_reader(&mut self) -> (&mut __Reader, &__Options) {
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

pub fn format_parsed_ty(name: &syn::Ident, generics: &syn::Generics) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut output_generics = generics.clone();
    output_generics.params.push(syn::parse_quote!(__Offset));
    let (_, output_generics, _) = output_generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ::#KRATE::parse::Parsed for #name #ty_generics #where_clause {
            type Output<__Offset> = #name #output_generics;
        }
    }
    .into()
}
