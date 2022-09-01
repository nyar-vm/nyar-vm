use super::*;

pub fn hello_world() -> WasmBuilder {
    let mut module = WasmBuilder::new("hello_world");

    let hello = "你好, 世界!".to_string();
    let hello_len = hello.len() as i32;

    module.insert_data(DataItem::utf8(WasmSymbol::new("hello"), hello));
    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "fd_write")
            .with_alias("file_descriptor_write")
            .with_input(vec![
                ParameterType::new("mode").with_type(WasmType::I32),
                ParameterType::new("offset_ptr").with_type(WasmType::I32),
                ParameterType::new("length_ptr").with_type(WasmType::I32),
                ParameterType::new("written").with_type(WasmType::I32),
            ])
            .with_output(vec![WasmType::I32]),
    );
    module.insert_function(
        FunctionType::new("print_str")
            .with_inputs(vec![
                ParameterType::new("offset").with_type(WasmType::I32),
                ParameterType::new("length").with_type(WasmType::I32),
            ])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![
                Operation::from(0),
                Operation::local_get("offset"),
                Operation::StoreVariable { r#type: WasmType::I32, offset: 0 },
                Operation::from(0),
                Operation::local_get("length"),
                Operation::StoreVariable { r#type: WasmType::I32, offset: 4 },
                Operation::CallFunction {
                    name: WasmSymbol::new("file_descriptor_write"),
                    input: vec![Operation::from(1), Operation::from(0), Operation::from(1), Operation::from(8)],
                },
            ]),
    );

    module.insert_function(
        FunctionType::new("_initialize")
            .with_inputs(vec![])
            .with_outputs(vec![])
            .with_operations(vec![
                Operation::CallFunction {
                    name: WasmSymbol::new("print_str"),
                    input: vec![Operation::from(65536), Operation::from(hello_len)],
                },
                Operation::Drop,
            ])
            .with_entry(true),
    );
    module
}

pub fn add_random_pi() -> WasmBuilder {
    let mut module = WasmBuilder::new("add_random_pi");

    module.insert_global(WasmVariable::f32("f32::pi", 3.14));

    module.insert_external(
        ExternalType::new("wasi_snapshot_preview1", "random_get")
            .with_alias("random_get")
            .with_input(vec![
                ParameterType::new("a").with_type(WasmType::I32),
                ParameterType::new("b").with_type(WasmType::I32),
            ])
            .with_output(vec![WasmType::I32]),
    );

    module.insert_function(
        FunctionType::new(WasmSymbol::new("sum_all"))
            .with_inputs(vec![
                ParameterType::new("a").with_type(WasmType::I32),
                ParameterType::new("b").with_type(WasmType::I32),
                ParameterType::new("c").with_type(WasmType::I32),
            ])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![
                Operation::CallFunction {
                    name: WasmSymbol::new("add_random"),
                    input: vec![Operation::Constant { value: WasmValue::F32(0.0) }],
                },
                Operation::Return {},
            ])
            .with_export(true),
    );

    module.insert_function(
        FunctionType::new(WasmSymbol::new("add_pi"))
            .with_inputs(vec![ParameterType::new("x").with_type(WasmType::F32)])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![Operation::NativeSum {
                r#type: WasmType::F32,
                terms: vec![Operation::Convert {
                    from: WasmType::F32,
                    into: WasmType::I32,
                    code: vec![Operation::NativeSum {
                        r#type: WasmType::F32,
                        terms: vec![Operation::global_get("f32::pi"), Operation::local_get("x")],
                    }],
                }],
            }])
            .with_export(true),
    );

    module.insert_function(
        FunctionType::new(WasmSymbol::new("_main"))
            .with_inputs(vec![])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![Operation::CallFunction {
                name: WasmSymbol::new("random_get"),
                input: vec![Operation::Constant { value: WasmValue::I32(1) }, Operation::Constant { value: WasmValue::I32(1) }],
            }])
            .with_export(true),
    );
    module
}

pub fn gc_types() -> WasmBuilder {
    let mut module = WasmBuilder::new("add_random_pi");

    module.insert_type(StructureType::new(WasmSymbol::new("Stable")).with_fields(vec![
        FieldType::new(WasmSymbol::new("a")).with_type(WasmType::U32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("c")).with_type(WasmType::Unicode).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("d")).with_type(WasmType::Any { nullable: false }).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("e")).with_type(WasmType::Any { nullable: true }).with_default(WasmValue::F32(2.0)),
    ]));

    module.insert_type(StructureType::new(WasmSymbol::new("a")).with_fields(vec![
        FieldType::new(WasmSymbol::new("a")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    ]));

    module.insert_type(ArrayType::new("core∷text∷UTF8Text", WasmType::I8));
    module.insert_type(ArrayType::new("core∷text∷UTF16Text", WasmType::I16));
    module.insert_type(ArrayType::new("core∷text∷UTF32Text", WasmType::I32));
    module
}
