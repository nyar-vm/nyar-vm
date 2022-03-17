#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

pub use crate::types::TypeItem;
use crate::{functions::FunctionItem, modules::ModuleBuilder};
use nyar_hir::{Identifier, NamedValue, Symbol};
pub use runner::run;
use wasm_encoder::{Function, Instruction};

mod builder;
pub mod helpers;
mod modules;
mod runner;
mod types;

mod functions;
mod values;

#[test]
fn test() {
    let mut module = ModuleBuilder::new();
    module.insert_global(NamedValue::i32(Identifier::from_iter(vec![Symbol::new("math"), Symbol::new("pi")]), 3));

    let locals = vec![];
    let mut body1 = Function::new(locals);
    body1.instruction(&Instruction::LocalGet(0));
    body1.instruction(&Instruction::LocalGet(1));
    body1.instruction(&Instruction::I32Add);
    body1.instruction(&Instruction::End);

    module.insert_function(FunctionItem {
        name: "add_ab".to_string(),
        export: false,
        typing: "Func".to_string(),
        body: body1.clone(),
    });

    let locals = vec![];
    let mut body2 = Function::new(locals);
    body2.instruction(&Instruction::LocalGet(0));
    body2.instruction(&Instruction::GlobalGet(0));
    body2.instruction(&Instruction::I32Add);
    body2.instruction(&Instruction::End);
    module.insert_function(FunctionItem {
        name: "add_ba".to_string(),
        export: true,
        typing: "Func".to_string(),
        body: body2.clone(),
    });

    let module = module.build().unwrap();
    let wat = wasmprinter::print_bytes(&module).expect("A");
    println!("{}", wat);
    run(&module).unwrap();
}
