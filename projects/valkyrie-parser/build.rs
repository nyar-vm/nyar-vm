#[macro_use]
extern crate quote;
extern crate pest_generator;
extern crate proc_macro;

use pest_generator::derive_parser;
use proc_macro::TokenStream;
use std::{fs::File, io::prelude::*, path::Path};

fn main() {
    let pest = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/valkyrie.pest"));
    let rs = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/pest_parser.rs"));

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
