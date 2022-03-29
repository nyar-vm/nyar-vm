use nyar_hir::{FunctionType, NyarType, NyarValue, Operation, ParameterType, Symbol, VariableKind};
use nyar_wasm::{ExternalType, ModuleBuilder, WasmVariable};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn test() {
    let mut module = ModuleBuilder::new(16);
    module.insert_global(WasmVariable::f32(Symbol::new("math.pi"), 3.14).with_public());

    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "fd_write")
            .with_input(vec![NyarType::I32, NyarType::I32, NyarType::I32, NyarType::I32])
            .with_output(vec![NyarType::I32]),
    );

    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "random_get")
            .with_input(vec![NyarType::I32, NyarType::I32])
            .with_output(vec![NyarType::I32]),
    );

    // module.insert_data(DataItem::utf8(Symbol::new("hello1"), "fuck world!".to_string()));
    // module.insert_data(DataItem::utf8(Symbol::new("hello2"), "我艹, 世界!".to_string()));
    // module.insert_data(DataItem::utf8(Symbol::new("hello3"), "くそったれ世界!".to_string()));

    // module.insert_type(StructureType::new(Symbol::new("Stable")).with_fields(vec![
    //     FieldType::new(Symbol::new("a"), NyarValue::F32(2.0)),
    //     FieldType::new(Symbol::new("b"), NyarValue::I32(1)),
    //     FieldType::new(Symbol::new("c"), NyarValue::Any),
    //     FieldType::new(Symbol::new("d"), NyarValue::Structure),
    //     FieldType::new(Symbol::new("e"), NyarValue::Array),
    // ]));
    //
    // module.insert_type(StructureType::new(Symbol::new("a")).with_fields(vec![
    //     FieldType::new(Symbol::new("a"), NyarValue::F32(2.0)),
    //     FieldType::new(Symbol::new("b"), NyarValue::I32(1)),
    // ]));

    // module.insert_type(ArrayType::new(Symbol::new("Bytes"), NyarType::I32));

    module.insert_function(
        FunctionType::new(Symbol::new("add_ab"))
            .with_inputs(vec![
                ParameterType { name: Symbol::new("a"), type_hint: NyarType::I32, span: Default::default() },
                ParameterType { name: Symbol::new("b"), type_hint: NyarType::I32, span: Default::default() },
            ])
            .with_outputs(vec![NyarType::I32])
            .with_operations(vec![
                Operation::NativeSum {
                    native: NyarType::I32,
                    terms: vec![
                        Operation::GetVariable { kind: VariableKind::Local, variable: Symbol::new("a") },
                        Operation::GetVariable { kind: VariableKind::Local, variable: Symbol::new("b") },
                        Operation::Convert {
                            from: NyarType::I32,
                            into: NyarType::I32,
                            code: vec![Operation::CallFunction {
                                name: Symbol::new("add_ba"),
                                input: vec![Operation::Constant { value: NyarValue::F32(0.0) }],
                            }],
                        },
                    ],
                },
                Operation::Drop,
                Operation::r#loop(
                    "for-1",
                    vec![
                        Operation::Constant { value: NyarValue::I32(0) },
                        Operation::Constant { value: NyarValue::I32(0) },
                        Operation::Drop,
                        Operation::Drop,
                        Operation::r#break("for-1"),
                    ],
                ),
                Operation::Constant { value: NyarValue::I32(0) },
                Operation::Return {},
            ])
            .with_public(),
    );

    module.insert_function(
        FunctionType::new(Symbol::new("add_ba"))
            .with_inputs(vec![ParameterType { name: Symbol::new("b"), type_hint: NyarType::F32, span: Default::default() }])
            .with_outputs(vec![NyarType::I32])
            .with_operations(vec![Operation::NativeSum {
                native: NyarType::F32,
                terms: vec![Operation::Convert {
                    from: NyarType::F32,
                    into: NyarType::I32,
                    code: vec![Operation::NativeSum {
                        native: NyarType::F32,
                        terms: vec![
                            Operation::GetVariable { kind: VariableKind::Global, variable: Symbol::new("math.pi") },
                            Operation::GetVariable { kind: VariableKind::Local, variable: Symbol::new("b") },
                        ],
                    }],
                }],
            }])
            .with_public(),
    );

    module.insert_function(
        FunctionType::new(Symbol::new("_start")).with_inputs(vec![]).with_outputs(vec![]).with_operations(vec![]),
    );

    let wast = module.build_module().unwrap().encode().unwrap();
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/valkyrie/runtime.wasm");
    let mut file = File::create(out).unwrap();
    file.write_all(&wast).unwrap();
}
