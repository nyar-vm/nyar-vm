use pest_generator::derive_parser;
use std::{fs::File, io::prelude::*, path::Path};

pub fn gen_valkyrie() {
    let pest = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "../../valkyrie-parser/valkyrie.pest"));
    let rs = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "../../valkyrie-parser/src/pest_parser.rs"));

    let derived = {
        let path = pest.to_string_lossy();
        let pest = quote! {
            #[grammar = #path]
            pub struct Valkyrie;
        };
        derive_parser(pest, false)
    };

    let mut file = File::create(rs).unwrap();

    let out = format!("pub struct Valkyrie;\n{}", derived);
    writeln!(file, "{}", out).unwrap();
}
