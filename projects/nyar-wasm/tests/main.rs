use nyar_wasm::{
    ArrayType, ExternalType, FieldType, FunctionType, ModuleBuilder, Operation, ParameterType, StructureType, VariableKind,
    WasmSymbol, WasmType, WasmValue, WasmVariable,
};
use std::{fs::File, io::Write, path::Path, process::Command};

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn test() {
    let mut module = ModuleBuilder::new(16);
    module.insert_global(WasmVariable::f32(WasmSymbol::new("math.pi"), 3.14).with_public());

    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "fd_write")
            .with_alias("file_descriptor_write")
            .with_input(vec![WasmType::I32, WasmType::I32, WasmType::I32, WasmType::I32])
            .with_output(vec![WasmType::I32]),
    );

    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "random_get")
            .with_alias("random_get")
            .with_input(vec![WasmType::I32, WasmType::I32])
            .with_output(vec![WasmType::I32]),
    );

    // module.insert_data(DataItem::utf8(Symbol::new("hello1"), "fuck world!".to_string()));
    // module.insert_data(DataItem::utf8(Symbol::new("hello2"), "我艹, 世界!".to_string()));
    // module.insert_data(DataItem::utf8(Symbol::new("hello3"), "くそったれ世界!".to_string()));

    module.insert_type(StructureType::new(WasmSymbol::new("Stable")).with_fields(vec![
        FieldType::new(WasmSymbol::new("a")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("c")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("d")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("e")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    ]));

    module.insert_type(StructureType::new(WasmSymbol::new("a")).with_fields(vec![
        FieldType::new(WasmSymbol::new("a")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    ]));

    module.insert_type(ArrayType::new("core∷text∷UTF8Text", WasmType::I8));
    module.insert_type(ArrayType::new("core∷text∷UTF16Text", WasmType::I16));
    module.insert_type(ArrayType::new("core∷text∷UTF32Text", WasmType::I32));

    module.insert_function(
        FunctionType::new(WasmSymbol::new("add_ab"))
            .with_inputs(vec![
                ParameterType::new("a").with_type(WasmType::I32),
                ParameterType::new("b").with_type(WasmType::I32),
                ParameterType::new("c").with_type(WasmType::I32),
            ])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![
                Operation::NativeSum {
                    native: WasmType::I32,
                    terms: vec![
                        Operation::local_get("a"),
                        Operation::local_get("b"),
                        Operation::Convert {
                            from: WasmType::I32,
                            into: WasmType::I32,
                            code: vec![Operation::CallFunction {
                                name: WasmSymbol::new("add_ba"),
                                input: vec![Operation::Constant { value: WasmValue::F32(0.0) }],
                            }],
                        },
                    ],
                },
                Operation::drop(1),
                Operation::r#loop(
                    "for-1",
                    vec![
                        Operation::Constant { value: WasmValue::I32(0) },
                        Operation::Constant { value: WasmValue::I32(0) },
                        Operation::drop(2),
                        Operation::r#break("for-1"),
                    ],
                ),
                Operation::Constant { value: WasmValue::I32(0) },
                Operation::Return {},
            ])
            .with_public(),
    );

    module.insert_function(
        FunctionType::new(WasmSymbol::new("add_ba"))
            .with_inputs(vec![ParameterType::new("b").with_type(WasmType::F32)])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![Operation::NativeSum {
                native: WasmType::F32,
                terms: vec![Operation::Convert {
                    from: WasmType::F32,
                    into: WasmType::I32,
                    code: vec![Operation::NativeSum {
                        native: WasmType::F32,
                        terms: vec![
                            Operation::GetVariable { kind: VariableKind::Global, variable: WasmSymbol::new("math.pi") },
                            Operation::GetVariable { kind: VariableKind::Local, variable: WasmSymbol::new("b") },
                        ],
                    }],
                }],
            }])
            .with_public(),
    );

    module.insert_function(
        FunctionType::new(WasmSymbol::new("__main"))
            .with_inputs(vec![])
            .with_outputs(vec![])
            .with_operations(vec![Operation::CallFunction {
                name: WasmSymbol::new("random_get"),
                input: vec![Operation::Constant { value: WasmValue::I32(1) }, Operation::Constant { value: WasmValue::I32(1) }],
            }])
            .with_entry()
            .with_private(),
    );

    let wast = module.build_module().unwrap().encode().unwrap();
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/valkyrie/runtime.wasm");
    let dir = out.parent().unwrap();
    if !dir.exists() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let mut file = File::create(out).unwrap();
    file.write_all(&wast).unwrap();
    let _ = Command::new("vcc").arg("build").output();
}
