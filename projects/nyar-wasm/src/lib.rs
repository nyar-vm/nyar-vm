#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

use crate::modules::{DataItem, ModuleBuilder};
pub use crate::types::TypeItem;
use nyar_hir::{
    FunctionExternalItem, FunctionItem, FunctionType, Identifier, NamedValue, NativeDataType, NyarType, NyarValue, Operation,
    Symbol,
};
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

    module.insert_function(
        FunctionItem::new(Identifier::from_iter(vec![Symbol::new("add_ab")]))
            .with_inputs(vec![NyarType::I32, NyarType::I32])
            .with_outputs(vec![NyarType::I32])
            .with_operations(vec![Operation::NativeSum {
                native: NativeDataType::I32,
                terms: vec![
                    Operation::LocalGet { index: 0 },
                    Operation::LocalGet { index: 1 },
                    Operation::Constant { value: NyarValue::I32(17) },
                ],
            }])
            .with_public(),
    );

    module.insert_function(
        FunctionItem::new(Identifier::from_iter(vec![Symbol::new("add_ba")]))
            .with_inputs(vec![NyarType::I32])
            .with_outputs(vec![NyarType::I32])
            .with_operations(vec![Operation::NativeSum {
                native: NativeDataType::I32,
                terms: vec![Operation::GlobalGet { index: 0 }, Operation::LocalGet { index: 0 }],
            }])
            .with_public(),
    );

    module.insert_external(
        FunctionExternalItem::new("wasi_snapshot_preview1", "fd_write")
            .with_input(vec![NyarType::I32, NyarType::I32, NyarType::I32, NyarType::I32])
            .with_output(vec![NyarType::I32]),
    );

    module.insert_function(
        FunctionItem::new(Identifier::from_iter(vec![Symbol::new("_start")]))
            .with_inputs(vec![])
            .with_outputs(vec![])
            .with_operations(vec![]),
    );

    let module = module.build().unwrap();
    let wat = wasmprinter::print_bytes(&module).expect("A");
    println!("{}", wat);
    run(&module).unwrap();
}
