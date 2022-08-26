use nyar_error::NyarError;
use nyar_wasm::{
    DataItem, ExternalType, FieldType, FunctionType, JumpBranch, JumpCondition, JumpTable, Operation, ParameterType,
    StructureType, WasmBuilder, WasmSymbol, WasmType, WasmValue, WasmVariable,
};
use std::{fs::File, io::Write, path::Path, process::Command};

#[test]
fn ready() {
    println!("it works!")
}

fn test_wasm() -> WasmBuilder {
    let mut module = WasmBuilder::new("我艹这他妈的什么鬼?");
    module.insert_global(WasmVariable::f32("f32::pi", 3.14));

    // module.insert_external(
    //     ExternalType::new("wasi_snapshot_preview1", "fd_write")
    //         .with_alias("file_descriptor_write")
    //         .with_input(vec![WasmType::I32, WasmType::I32, WasmType::I32, WasmType::I32])
    //         .with_output(vec![WasmType::I32]),
    // );
    //
    // module.insert_external(
    //     ExternalType::new("wasi_snapshot_preview1", "random_get")
    //         .with_alias("random_get")
    //         .with_input(vec![WasmType::I32, WasmType::I32])
    //         .with_output(vec![WasmType::I32]),
    // );

    // module.insert_data(DataItem::utf8(Symbol::new("hello1"), "fuck world!".to_string()));
    // module.insert_data(DataItem::utf8(Symbol::new("hello2"), "我艹, 世界!".to_string()));
    // module.insert_data(DataItem::utf8(Symbol::new("hello3"), "くそったれ世界!".to_string()));

    // module.insert_type(StructureType::new(WasmSymbol::new("Stable")).with_fields(vec![
    //     FieldType::new(WasmSymbol::new("a")).with_type(WasmType::U32).with_default(WasmValue::F32(2.0)),
    //     FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    //     FieldType::new(WasmSymbol::new("c")).with_type(WasmType::Unicode).with_default(WasmValue::F32(2.0)),
    //     // FieldType::new(WasmSymbol::new("d")).with_type(WasmType::Any { nullable: false }).with_default(WasmValue::F32(2.0)),
    //     // FieldType::new(WasmSymbol::new("e")).with_type(WasmType::Any { nullable: true }).with_default(WasmValue::F32(2.0)),
    // ]));
    //
    // module.insert_type(StructureType::new(WasmSymbol::new("a")).with_fields(vec![
    //     FieldType::new(WasmSymbol::new("a")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    //     FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    // ]));

    // module.insert_type(ArrayType::new("core∷text∷UTF8Text", WasmType::I8));
    // module.insert_type(ArrayType::new("core∷text∷UTF16Text", WasmType::I16));
    // module.insert_type(ArrayType::new("core∷text∷UTF32Text", WasmType::I32));

    // module.insert_function(
    //     FunctionType::new(WasmSymbol::new("sum_all"))
    //         .with_inputs(vec![
    //             ParameterType::new("a").with_type(WasmType::I32),
    //             ParameterType::new("b").with_type(WasmType::I32),
    //             ParameterType::new("c").with_type(WasmType::I32),
    //         ])
    //         .with_outputs(vec![WasmType::I32])
    //         .with_operations(vec![
    //             Operation::NativeSum {
    //                 native: WasmType::I32,
    //                 terms: vec![
    //                     Operation::local_get("a"),
    //                     Operation::local_get("b"),
    //                     Operation::CallFunction {
    //                         name: WasmSymbol::new("add_random"),
    //                         input: vec![Operation::Constant { value: WasmValue::F32(0.0) }],
    //                     },
    //                     // test_if_1(),
    //                     test_if_2(),
    //                     test_if_3(),
    //                     test_switch(),
    //                 ],
    //             },
    //             Operation::Return {},
    //         ])
    //         .with_export(true),
    // );
    //
    module.insert_function(
        FunctionType::new(WasmSymbol::new("add_pi"))
            .with_inputs(vec![ParameterType::new("x").with_type(WasmType::F32)])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![Operation::NativeSum {
                native: WasmType::F32,
                terms: vec![Operation::Convert {
                    from: WasmType::F32,
                    into: WasmType::I32,
                    code: vec![Operation::NativeSum {
                        native: WasmType::F32,
                        terms: vec![Operation::global_get("f32::pi"), Operation::local_get("x")],
                    }],
                }],
            }])
            .with_export(true),
    );

    module.insert_function(
        FunctionType::new(WasmSymbol::new("_start"))
            .with_inputs(vec![])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![
            //     Operation::CallFunction {
            //     name: WasmSymbol::new("random_get"),
            //     input: vec![Operation::Constant { value: WasmValue::I32(1) }, Operation::Constant { value: WasmValue::I32(1) }],
            // }
            ])
            .with_export(true),
    );
    module
}

#[test]
fn test() -> Result<(), NyarError> {
    let debug = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/valkyrie");
    if !debug.exists() {
        std::fs::create_dir_all(&debug).unwrap();
    }
    let _ = test_wasm().build_component(debug.join("com.wasm")).unwrap();
    let _ = test_wasm().build_module(debug.join("mod.wasm")).unwrap();
    let o = Command::new("valor").arg("build").output();
    println!("{o:?}");
    Ok(())
}

#[test]
fn hello() {
    const text: &str = "hello world!";
    let mut module = WasmBuilder::new("hello");
    module.insert_global(WasmVariable::i32("a", 42).with_immutable());
    module.insert_global(WasmVariable::f32("pi", 3.14).with_immutable());
    module.insert_data(DataItem::utf8(WasmSymbol::new("hello"), text.to_string()));
    module.insert_function(
        FunctionType::new(WasmSymbol::new("__main"))
            .with_inputs(vec![])
            .with_outputs(vec![])
            .with_operations(vec![
                // Operation::CallFunction {
                //     name: WasmSymbol::new("file_descriptor_write"),
                //     input: vec![Operation::from(1), Operation::from(0), Operation::from(text.len() as i32), Operation::from(0)],
                // },
                // Operation::Drop,
            ])
            .with_entry(),
    );

    let wast = module.as_module().encode().unwrap();
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/valkyrie/hello.wasm");
    let dir = out.parent().unwrap();
    if !dir.exists() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let mut file = File::create(out).unwrap();
    file.write_all(&wast).unwrap();
    let o = Command::new("valor").arg("build").output();
    println!("{o:?}")
}

fn test_if_1() -> Operation {
    Operation::if_then(
        vec![Operation::local_get("a")],
        vec![Operation::Constant { value: WasmValue::F32(1.618) }, Operation::drop(1)],
    )
}
fn test_if_2() -> Operation {
    Operation::JumpBranch(
        JumpBranch::if_then_else(
            vec![Operation::local_get("a")],
            vec![Operation::from(1), Operation::drop(0)],
            vec![Operation::from(2), Operation::from(3), Operation::drop(1)],
        )
        .with_return_type(vec![WasmType::I32]),
    )
}
fn test_if_3() -> Operation {
    Operation::JumpTable(JumpTable {
        branches: vec![
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    native: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(1)],
                }],
                action: vec![Operation::from(2)],
            },
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    native: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(3)],
                }],
                action: vec![Operation::from(3)],
            },
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    native: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(5)],
                }],
                action: vec![Operation::from(5)],
            },
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    native: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(7)],
                }],
                action: vec![Operation::from(7)],
            },
        ],
        default: vec![Operation::Unreachable],
        r#return: vec![WasmType::I32],
    })
}

fn test_switch() -> Operation {
    Operation::JumpTable(JumpTable {
        branches: vec![
            JumpCondition { condition: vec![Operation::local_get("a")], action: vec![Operation::from(1)] },
            JumpCondition { condition: vec![Operation::local_get("b")], action: vec![Operation::from(2)] },
            JumpCondition { condition: vec![Operation::local_get("c")], action: vec![Operation::from(3)] },
        ],
        default: vec![Operation::from(4)],
        r#return: vec![WasmType::I32],
    })
}

fn test_loop1() -> Operation {
    Operation::r#loop("for-1", vec![Operation::from(0), Operation::from(0), Operation::drop(2), Operation::r#break("for-1")])
}
