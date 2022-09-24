use nyar_wasm::{
    DataItem, ExternalType, FieldType, FunctionType, Operation, StructureItem, WasmBuilder, WasmExternalName, WasmParameter,
    WasmSymbol, WasmType, WasmValue,
};
use semver::Version;

pub fn hello_world() -> WasmBuilder {
    let mut module = WasmBuilder::new("hello_world");

    let hello = "你好, 世界!".to_string();
    let hello_len = hello.len() as i32;

    module.register_data(DataItem::utf8(WasmSymbol::new("hello"), hello));
    module.register(
        ExternalType::new("wasi_snapshot_preview1", "fd_write")
            .with_alias("file_descriptor_write")
            .with_input(vec![
                WasmParameter::new("mode").with_type(WasmType::I32),
                WasmParameter::new("offset_ptr").with_type(WasmType::I32),
                WasmParameter::new("length_ptr").with_type(WasmType::I32),
                WasmParameter::new("written").with_type(WasmType::I32),
            ])
            .with_output(vec![WasmType::I32]),
    );

    module.register(
        FunctionType::new("print_str")
            .with_inputs(vec![
                WasmParameter::new("offset").with_type(WasmType::I32),
                WasmParameter::new("length").with_type(WasmType::I32),
            ])
            .with_output(WasmType::I32)
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

    module.register(
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

    // module.register_global(WasmVariable::f32("f32::pi", 3.14));

    // module.register(
    //     ExternalType::new("wasi_snapshot_preview1", "random_get")
    //         .with_alias("random_get")
    //         .with_input(vec![
    //             WasmParameter::new("a").with_type(WasmType::I32),
    //             WasmParameter::new("b").with_type(WasmType::I32),
    //         ])
    //         .with_output(vec![WasmType::I32]),
    // );
    //

    module.register(
        FunctionType::new(WasmSymbol::new("const-42"))
            .with_inputs(vec![])
            .with_outputs(vec![
                WasmParameter::new("r0").with_type(WasmType::I64),
                WasmParameter::new("r1").with_type(WasmType::I64),
                WasmParameter::new("r2").with_type(WasmType::I64),
                WasmParameter::new("r3").with_type(WasmType::I64),
            ])
            .with_operations(vec![
                Operation::from(42i64),
                Operation::from(42i64),
                Operation::from(42i64),
                Operation::from(42i64),
                Operation::Return {},
            ])
            .with_export(WasmExternalName::create("a-two").with_project("org", "package").with_version(Version::new(0, 1, 0))),
    );

    module.register(
        FunctionType::new(WasmSymbol::new("const88"))
            .with_inputs(vec![])
            .with_output(WasmType::I64)
            .with_operations(vec![Operation::Constant { value: WasmValue::I64(88) }, Operation::Return {}])
            .auto_export(true),
    );

    // module.register(
    //     FunctionType::new(WasmSymbol::new("sum_all"))
    //         .with_inputs(vec![
    //             WasmParameter::new("a").with_type(WasmType::I32),
    //             WasmParameter::new("b").with_type(WasmType::I32),
    //             WasmParameter::new("c").with_type(WasmType::I32),
    //         ])
    //         .with_outputs(vec![WasmType::I32])
    //         .with_operations(vec![
    //             Operation::CallFunction {
    //                 name: WasmSymbol::new("add_random"),
    //                 input: vec![Operation::Constant { value: WasmValue::F32(0.0) }],
    //             },
    //             Operation::Return {},
    //         ])
    //         .auto_export(true),
    // );

    // module.register(
    //     FunctionType::new(WasmSymbol::new("add_pi"))
    //         .with_inputs(vec![WasmParameter::new("x").with_type(WasmType::F32)])
    //         .with_outputs(vec![WasmType::I32])
    //         .with_operations(vec![Operation::NativeSum {
    //             r#type: WasmType::F32,
    //             terms: vec![Operation::Convert {
    //                 from: WasmType::F32,
    //                 into: WasmType::I32,
    //                 code: vec![Operation::NativeSum {
    //                     r#type: WasmType::F32,
    //                     terms: vec![Operation::global_get("f32::pi"), Operation::local_get("x")],
    //                 }],
    //             }],
    //         }])
    //         .auto_export(true),
    // );

    // module.register(
    //     FunctionType::new(WasmSymbol::new("_main"))
    //         .with_inputs(vec![])
    //         .with_outputs(vec![WasmType::I32])
    //         .with_operations(vec![Operation::CallFunction {
    //             name: WasmSymbol::new("random_get"),
    //             input: vec![Operation::Constant { value: WasmValue::I32(1) }, Operation::Constant { value: WasmValue::I32(1) }],
    //         }])
    //         .auto_export(true),
    // );
    module
}

pub fn gc_types() -> WasmBuilder {
    let mut module = WasmBuilder::new("add_random_pi");

    module.register(StructureItem::new("Stable").with_fields(vec![
        FieldType::new(WasmSymbol::new("a")).with_type(WasmType::U32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("c")).with_type(WasmType::Unicode).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("d")).with_type(WasmType::Any { nullable: false }).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("e")).with_type(WasmType::Any { nullable: true }).with_default(WasmValue::F32(2.0)),
    ]));

    module.register(StructureItem::new(WasmSymbol::new("a")).with_fields(vec![
        FieldType::new(WasmSymbol::new("a")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
        FieldType::new(WasmSymbol::new("b")).with_type(WasmType::F32).with_default(WasmValue::F32(2.0)),
    ]));

    // module.insert_structure(&ArrayType::new("core∷text∷UTF8Text", WasmType::I8));
    // module.insert_structure(ArrayType::new("core∷text∷UTF16Text", WasmType::I16));
    // module.insert_structure(ArrayType::new("core∷text∷UTF32Text", WasmType::I32));
    module
}
