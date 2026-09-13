use std::collections::HashSet;
use crate::{VersionList};

#[derive(Clone)]
pub struct Child {
    pub name: syn::Ident,
    pub version_num: HashSet<syn::LitInt>,
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

pub struct ChildList {
    pub inner: Vec<Child>,
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
