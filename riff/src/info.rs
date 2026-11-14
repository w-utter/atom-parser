use atom_parser::{FourCC, make_atom};

// info specific entries
make_atom! {
    #[derive(Debug)]
    struct StringData {
        #[payload]
        string: String,
    }

    #[derive(Debug)]
    struct InfoEntry {
        #[tag]
        kind: FourCC,
        #[payload_length(padding = 2)]
        size: u32,
        #[tagged]
        entry: enum InfoEntry {
            crate::List as GenericList,

            #[override_fourcc("IARL")]
            StringData as ArchivalLocation,
            #[override_fourcc("IART")]
            StringData as Artist,
            #[override_fourcc("ICMS")]
            StringData as Comissioned,
            #[override_fourcc("ICMT")]
            StringData as Comments,
            #[override_fourcc("ICOP")]
            StringData as Copyright,
            #[override_fourcc("ICRD")]
            StringData as CreationDate,
            #[override_fourcc("ICRP")]
            StringData as Cropped,
            #[override_fourcc("IDIM")]
            StringData as Dimensions,
            #[override_fourcc("IDPI")]
            StringData as DotsPerInch,
            #[override_fourcc("IENG")]
            StringData as Engineer,
            #[override_fourcc("IGNR")]
            StringData as Genre,
            #[override_fourcc("IKEY")]
            StringData as Keywords,
            #[override_fourcc("ILGT")]
            StringData as Lightness,
            #[override_fourcc("IMED")]
            StringData as Medium,
            #[override_fourcc("INAM")]
            StringData as Name,
            #[override_fourcc("IPLT")]
            StringData as PaletteSetting,
            #[override_fourcc("IPRD")]
            StringData as Product,
            #[override_fourcc("ISBJ")]
            StringData as Subject,
            #[override_fourcc("ISFT")]
            StringData as Software,
            #[override_fourcc("ISHP")]
            StringData as Sharpness,
            #[override_fourcc("ISRC")]
            StringData as Source,
            #[override_fourcc("ISRF")]
            StringData as SourceForm,
            #[override_fourcc("ITCH")]
            StringData as Technician,
        }
    }

    #[derive(Debug)]
    #[tag(FourCC, FourCC::new(*b"INFO"))]
    struct InfoList {
        #[trailing_array]
        entries: InfoEntry,
    }
}
