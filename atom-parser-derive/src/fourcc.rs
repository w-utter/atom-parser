#[derive(Clone)]
pub struct FourCC([u8; 4]);

impl FourCC {
    pub fn from_syn(attr: syn::Attribute) -> syn::parse::Result<Self> {
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

    pub fn try_as_mod(&self) -> Option<&str> {
        let str = str::from_utf8(&self.0).ok()?;

        if !str
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit())
        {
            return None;
        }
        Some(str)
    }

    pub fn as_ident(&self) -> syn::Ident {
        if let Some(name) = self.try_as_mod() {
            syn::Ident::new(name, proc_macro2::Span::call_site())
        } else {
            let num = u32::from_be_bytes(self.0);
            let name = format!("atom_{:#x}", num);
            syn::Ident::new(&name, proc_macro2::Span::call_site())
        }
    }

    pub fn as_fcc(&self) -> [u8; 4] {
        self.0
    }
}
