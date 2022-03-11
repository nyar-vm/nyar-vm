// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

pub use crate::types::TypeBuilder;
use crate::{builder::ModuleBuilder, modules::GlobalValue};
pub use runner::run;

mod builder;
mod modules;
mod runner;
mod types;

#[test]
fn test() {
    let mut module = ModuleBuilder::new();
    module.insert_global(GlobalValue::new_f32("pi", std::f32::consts::PI));

    let module = module.build();
    let wat = wasmprinter::print_bytes(&module).expect("A");
    println!("{}", wat);
    run(&module).unwrap();
}
