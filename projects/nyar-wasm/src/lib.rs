#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

use crate::modules::{DataItem, ModuleBuilder};
use nyar_hir::{
    ArrayType, ExternalType, FieldType, FunctionType, Identifier, NamedValue, NativeDataType, NyarType, NyarValue, Operation,
    StructureType, Symbol,
};
pub use runner::run;

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
    module.insert_global(NamedValue::f32(Symbol::new("math.pi"), 3.14).with_public());
    module.insert_data(DataItem::utf8(Symbol::new("math1"), "hello world".to_string()));
    module.insert_data(DataItem::utf8(Symbol::new("math2"), "fuck world 中文".to_string()));
    module.insert_data(DataItem::utf8(Symbol::new("math3"), "fuck world 中文1".to_string()));
    module.insert_data(DataItem::utf8(Symbol::new("math4"), "fuck world 中文2".to_string()));

    module.insert_type(StructureType::new(Symbol::new("Stable")).with_fields(vec![
        FieldType::new(Symbol::new("a"), NyarValue::F32(2.0)),
        FieldType::new(Symbol::new("b"), NyarValue::I32(1)),
        FieldType::new(Symbol::new("c"), NyarValue::Any),
        FieldType::new(Symbol::new("d"), NyarValue::Structure),
        FieldType::new(Symbol::new("e"), NyarValue::Array),
    ]));

    module.insert_type(StructureType::new(Symbol::new("a")).with_fields(vec![
        FieldType::new(Symbol::new("a"), NyarValue::F32(2.0)),
        FieldType::new(Symbol::new("b"), NyarValue::I32(1)),
    ]));

    module.insert_type(ArrayType::new(Symbol::new("Bytes"), NyarType::I32));

    module.insert_function(
        FunctionType::new(Symbol::new("add_ab"))
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
        FunctionType::new(Symbol::new("add_ba"))
            .with_inputs(vec![NyarType::I32])
            .with_outputs(vec![NyarType::I32])
            .with_operations(vec![Operation::NativeSum {
                native: NativeDataType::I32,
                terms: vec![Operation::GlobalGet { index: 0 }, Operation::LocalGet { index: 0 }],
            }])
            .with_public(),
    );

    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "fd_write")
            .with_input(vec![NyarType::I32, NyarType::I32, NyarType::I32, NyarType::I32])
            .with_output(vec![NyarType::I32]),
    );

    module.insert_function(
        FunctionType::new(Symbol::new("_start")).with_inputs(vec![]).with_outputs(vec![]).with_operations(vec![]),
    );

    let wast = module.build_module().unwrap().encode().unwrap();
    let wat = wasmprinter::print_bytes(&wast).expect("A");
    println!("{}", wat);

    run(&wast).unwrap();
}
