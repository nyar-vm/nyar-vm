#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

use crate::modules::{DataItem, ModuleBuilder};
pub use crate::types::TypeItem;
use nyar_hir::{FunctionItem, FunctionType, Identifier, NamedValue, NyarType, Symbol};
pub use runner::run;
use wasm_encoder::{Function, Instruction, ValType};

mod builder;
pub mod helpers;
mod modules;
mod runner;
mod types;

mod functions;
mod values;

#[test]
fn test() {
    let mut module = ModuleBuilder::new(16);
    module.insert_global(NamedValue::i32(Identifier::from_iter(vec![Symbol::new("math"), Symbol::new("pi")]), 3).with_public());
    module.insert_data(DataItem::utf8(Identifier::from_iter(vec![Symbol::new("math1")]), "hello world".to_string()));
    module.insert_data(DataItem::utf8(Identifier::from_iter(vec![Symbol::new("math2")]), "fuck world 中文".to_string()));
    module.insert_data(DataItem::utf8(Identifier::from_iter(vec![Symbol::new("math3")]), "fuck world 中文1".to_string()));
    module.insert_data(DataItem::utf8(Identifier::from_iter(vec![Symbol::new("math4")]), "fuck world 中文2".to_string()));

    let locals = vec![(1, ValType::I32)];
    let mut body1 = Function::new(locals);
    body1.instruction(&Instruction::I32Const(10));
    body1.instruction(&Instruction::LocalSet(2));
    body1.instruction(&Instruction::LocalGet(0));
    body1.instruction(&Instruction::LocalGet(1));
    body1.instruction(&Instruction::I32Add);
    body1.instruction(&Instruction::LocalGet(2));
    body1.instruction(&Instruction::I32Add);
    body1.instruction(&Instruction::End);

    module.insert_function(FunctionItem {
        namepath: "add_ab".to_string(),
        export: true,
        entry: false,
        typing: FunctionType {
            name: Identifier::from_iter(vec![]),
            input: vec![NyarType::I32, NyarType::I32],
            output: vec![NyarType::I32],
        },
        body: body1.clone(),
    });

    let locals = vec![];
    let mut body2 = Function::new(locals);
    body2.instruction(&Instruction::LocalGet(0));
    body2.instruction(&Instruction::GlobalGet(0));
    body2.instruction(&Instruction::I32Add);
    body2.instruction(&Instruction::End);
    module.insert_function(FunctionItem {
        namepath: "add_ba".to_string(),
        export: true,
        entry: false,
        typing: FunctionType { name: Identifier::from_iter(vec![]), input: vec![NyarType::I32], output: vec![NyarType::I32] },
        body: body2.clone(),
    });

    let locals = vec![];
    let mut body2 = Function::new(locals);
    body2.instruction(&Instruction::End);
    module.insert_function(FunctionItem {
        namepath: "_start".to_string(),
        export: false,
        entry: false,
        typing: FunctionType { name: Identifier::from_iter(vec![]), input: vec![], output: vec![] },
        body: body2.clone(),
    });

    let module = module.build().unwrap();
    let wat = wasmprinter::print_bytes(&module).expect("A");
    println!("{}", wat);
    run(&module).unwrap();
}
