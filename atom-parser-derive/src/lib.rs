extern crate proc_macro;
use proc_macro::TokenStream;

#[proc_macro]
pub fn make_atom(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::Item);

    use syn::{Item, Fields, spanned::Spanned};
    use quote::quote;

    enum FieldKind {
        Normal,
        DynamicArray(syn::Type),
        Reserved,
    }

    match &input {
        Item::Struct(data) => {
            match &data.fields {
                Fields::Named(n) => {
                    let async_impl = n.named.iter().map(|field| {
                    });

                    let mut trailing_iterator = None;
                    let mut trailing_payload = None;
                    let mut children = None;

                    let fields = n.named.iter().filter_map(|field| {
                        for attr in &field.attrs {
                            if attr.path().is_ident("trailing_iterator") {
                                if trailing_iterator.is_some() {
                                    todo!("error for duplicate trailing iterators");
                                    //return TokenStream::from()
                                }
                                trailing_iterator = Some(field);
                                return None;
                            } else if attr.path().is_ident("trailing_payload") {
                                if trailing_payload.is_some() {
                                    todo!("error for duplicate trailing payloads");
                                }
                                trailing_payload = Some(field);
                                return None;
                            } else if attr.path().is_ident("dynamic_array") {
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
                                }).unwrap();

                                let size_ty = size_ty.expect("no type specified for size of dynamic array");
                                let zero_relative = zero_relative.expect("zero-relative is not specified for size of dynamic array");

                                let name = &field.ident;
                                let ty = &field.ty;

                                let new_field: syn::Field = syn::parse_quote! {
                                    #name: DynamicArray<#size_ty, #ty, #zero_relative>
                                };

                                return Some((new_field, FieldKind::DynamicArray(ty.clone())));
                            } else if attr.path().is_ident("reserved") {
                                return Some((field.clone(), FieldKind::Reserved));
                            } else if attr.path().is_ident("children") {
                                if children.is_some() {
                                    todo!("duplicate children")
                                }
                                children = Some(field);
                                return None;
                            }
                        }
                        Some((field.clone(), FieldKind::Normal))
                    }).collect::<Vec<_>>();


                    let trailing_count = u32::from(trailing_payload.is_some()) + u32::from(trailing_iterator.is_some()) + u32::from(children.is_some());
                    if trailing_count > 1 {
                        panic!("cannot have multiple trailing fields");
                    }

                    let sync_impl = {
                        let parsing = fields.iter().map(|(field, kind)| {
                            let field_ty = &field.ty;
                            let field_name = &field.ident;

                            if matches!(kind, FieldKind::Reserved) {
                                quote!{
                                    let mut _reserved = [0; core::mem::size_of::<#field_ty>()];
                                    reader.read(&mut _reserved)?;
                                    // TODO: optionally validate reserved fields are 0
                                }
                            } else {
                                quote!{
                                    let #field_name = <#field_ty>::parse(reader, options)?;
                                }
                            }
                        });

                        let collection = fields.iter().filter_map(|(field, kind)| {
                            if matches!(kind, FieldKind::Reserved) {
                                None
                            } else {
                                let field_name = &field.ident;
                                Some(quote!{
                                    #field_name,
                                })
                            }
                        });

                        quote!{
                            fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                                #(#parsing)*
                                Ok(Self {
                                    #(#collection)*
                                })
                            }
                        }
                    };

                    let struct_fields = fields.iter().filter_map(|(field, kind)| {
                        if matches!(kind, FieldKind::Reserved) {
                            None
                        } else {
                            let field_name = &field.ident;
                            let field_ty = &field.ty;

                            Some(quote!{
                                pub #field_name: #field_ty
                            })
                        }
                    });

                    let struct_name = &data.ident;

                    let atom = data.attrs.iter().find(|attr| attr.path().is_ident("atom"));

                    let atom_impl = atom.cloned().map(|attr| {
                        if let Ok(bstr) = attr.parse_args::<syn::LitByteStr>() {
                            let fcc = bstr.value();

                            if fcc.len() != 4 {
                                let fcc = String::from_utf8_lossy_owned(fcc);
                                panic!("FCC `b{fcc:?}` has len {}, but must be len 4", fcc.len());
                            } else {
                                quote!{
                                    const FCC: FourCC = {
                                        let lit = #bstr;
                                        let fcc: [u8; 4] = [lit[0], lit[1], lit[2], lit[3]];
                                        FourCC(fcc)
                                    };
                                }
                            }
                        } else if let Ok(str) = attr.parse_args::<syn::LitStr>() {
                            let fcc = str.value();

                            if fcc.len() != 4 {
                                panic!("FCC `{fcc:?}` has len {}, but must be len 4", fcc.len());
                            } else {
                                quote! {
                                    const FCC: FourCC = {
                                        let lit = #fcc.as_bytes();
                                        let fcc: [u8; 4] = [lit[0], lit[1], lit[2], lit[3]];
                                        FourCC(fcc)
                                    };
                                }
                            }
                        } else {
                            panic!("fcc must be in the pattern of bstr or str, e.g `b\"ftyp\"` or `\"ftyp\"`")
                        }
                    }).map(|fcc| {
                        quote! {
                            impl Atom for #struct_name {
                                #fcc
                            }
                        }
                    });

                    // FIXNE: lifetimes for all of this can probably be cleaned up
                    let dynamic_array_fns = fields.iter().filter_map(|(field, kind)| {
                        if let FieldKind::DynamicArray(ty) = kind {
                            Some((field, ty))
                        } else {
                            None
                        }
                    }).map(|(dynamic_arr, arr_ty)| {
                        let arr_name = &dynamic_arr.ident;
                        quote! {
                            pub fn #arr_name<'a, R: Reader>(&'a self, reader: &'a mut R, opts: &'a ParseOptions) -> impl Iterator<Item = Result<#arr_ty, ParseError>> + 'a {
                                DynamicArrayIter {
                                    arr: &self.#arr_name,
                                    reader,
                                    opts,
                                }
                            }
                        }
                    });

                    let trailing_iter_fn = trailing_iterator.map(|trailing| {
                        let fn_name = &trailing.ident;
                        let iterator_ty = &trailing.ty;
                        // TODO: async equivalent
                        quote! {
                            pub fn #fn_name<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> impl Iterator<Item = Result<#iterator_ty, ParseError>> + 'a {
                                TrailingIterator {
                                    reader,
                                    opts,
                                    _pd: core::marker::PhantomData,
                                }
                            }
                        }
                    });

                    let child_fn = children.map(|_| {
                        let atom_fcc = atom.expect("need atom impl (fcc) to have children").parse_args::<syn::LitStr>().expect("could not parse str of fcc").value();
                        let atom = syn::Ident::new(&atom_fcc, proc_macro2::Span::call_site());

                        quote! {
                            pub fn children<'a, R: Reader>(&self, reader: &'a mut R, opts: &'a ParseOptions) -> impl Iterator<Item = Result<#atom::Child, ParseError>> + 'a {
                                ChildrenIter {
                                    _pd: core::marker::PhantomData,
                                    reader,
                                    opts,
                                }
                            }
                        }
                    });

                    TokenStream::from(quote!{
                        #[derive(Debug)]
                        struct #struct_name {
                            #(#struct_fields,)*
                        }

                        impl Parse for #struct_name {
                            #sync_impl

                            async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                                todo!()
                            }
                        }

                        impl #struct_name {
                            #trailing_iter_fn
                            #(#dynamic_array_fns)*
                            #child_fn
                        }

                        #atom_impl
                    })
                }
                _ => TokenStream::from(
                    syn::Error::new(
                        input.span(),
                        "structs need named fields",
                    )
                    .to_compile_error(),
                )
            }
        }
        Item::Enum(e) => {
            if e.ident == "Children" {
                let mut atom_fcc = None;
                for attr in &e.attrs {
                    if attr.path().is_ident("atom") {
                        atom_fcc = Some(attr.parse_args::<syn::Ident>().unwrap());
                    }
                }
                let atom_fcc = atom_fcc.expect("no fcc for what these children belong to");

                let sync_impl = {
                    let parse = e.variants.iter().map(|v| {
                        let name = &v.ident;
                        quote!{
                            <#name as Atom>::FCC => {
                                let atom = <#name as Parse>::parse(&mut r, options)?;
                                Child::#name(atom)
                            }
                        }
                    });

                    quote! {
                        fn parse<T: Reader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {

                            loop {
                                let atom = AtomHeader::parse(reader, options)?;
                                let mut r = TrailingReader::new(&mut*reader, atom.size.size as _);

                                return Ok(match atom.fcc {
                                    #(#parse)*
                                    missed => {
                                        // FIXME: other behaviours for missed atoms ?
                                        r.seek_remaining()?;
                                        Child::Unsupported(missed)
                                    }
                                })
                            }
                        }
                    }
                };

                let variants = e.variants.iter();

                TokenStream::from(quote! {
                    // FIXME: have the mod name be based on the atoms fcc
                    pub mod #atom_fcc {
                        use super::*;
                        pub enum Child {
                            #(
                                #variants(#variants),
                            )*
                            Unsupported(FourCC),
                        }

                        impl Parse for Child {
                            #sync_impl
                            async fn parse_async<T: AsyncReader>(reader: &mut T, options: &ParseOptions) -> Result<Self, ParseError> {
                                todo!()
                            }
                        }
                    }
                })
            } else {
                todo!("non children enum")
            }
        }
        _ => TokenStream::from(
            syn::Error::new(
                input.span(),
                "Only structs with named field sor enums  can be derived",
            )
            .to_compile_error(),
        )
    }
}

/*
#[proc_macro_derive(Atom, attributes(atom, fcc, size))]
pub fn derive_atom(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

}
*/

