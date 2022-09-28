use nyar_wasm::{
    DataItem, FieldType, FunctionType, ImportFunction, Operation, StructureItem, StructureType, WasmBuilder, WasmExternalName,
    WasmParameter, WasmSymbol, WasmType, WasmValue,
};
use semver::Version;

pub fn hello_world() -> WasmBuilder {
    let mut module = WasmBuilder::new("hello_world");

    let hello = "你好, 世界!".to_string();
    let hello_len = hello.len() as i32;

    module.register_data(DataItem::utf8(WasmSymbol::new("hello"), hello));
    module.register(
        ImportFunction::new("wasi_snapshot_preview1", "fd_write").with_local("file_descriptor_write").with_input(vec![
            WasmParameter::new("mode").with_type(WasmType::I32),
            WasmParameter::new("offset_ptr").with_type(WasmType::I32),
            WasmParameter::new("length_ptr").with_type(WasmType::I32),
            WasmParameter::new("written").with_type(WasmType::I32),
        ]), // .with_output(vec![WasmType::I32]),
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
    let mut module = WasmBuilder::new("test_tuples");

    // module.register(
    //     FunctionType::new(WasmSymbol::new("tuple1"))
    //         .with_inputs(vec![])
    //         .with_output(WasmType::I64)
    //         .with_operations(vec![Operation::from(0i64), Operation::Return {}])
    //         .auto_export(true),
    // );

    module.register(
        FunctionType::new(WasmSymbol::new("tuple2"))
            .with_inputs(vec![])
            .with_outputs(vec![
                WasmParameter::new("r0").with_type(WasmType::I64),
                WasmParameter::new("r1").with_type(WasmType::I64),
            ])
            .with_operations(vec![Operation::from(0i64), Operation::from(1i64), Operation::Return {}])
            .auto_export(true),
    );

    // module.register(
    //     FunctionType::new(WasmSymbol::new("tuple3"))
    //         .with_inputs(vec![])
    //         .with_outputs(vec![
    //             WasmParameter::new("r0").with_type(WasmType::I64),
    //             WasmParameter::new("r1").with_type(WasmType::I64),
    //             WasmParameter::new("r2").with_type(WasmType::I64),
    //         ])
    //         .with_operations(vec![Operation::from(0i64), Operation::from(1i64), Operation::from(2i64), Operation::Return {}])
    //         .auto_export(true),
    // );

    module
}

pub fn import_random() -> WasmBuilder {
    let mut module = WasmBuilder::new("test_import");
    let version = Version::new(0, 2, 0);

    module.register(
        ImportFunction::new(
            WasmExternalName::create("random").with_project("wasi", "random").with_version(version.clone()),
            "get-random-u64",
        )
        .with_local("random_seed_safe")
        .with_output(WasmType::U64),
    );
    module.register(
        ImportFunction::new(
            WasmExternalName::create("insecure").with_project("wasi", "random").with_version(version.clone()),
            "get-insecure-random-u64",
        )
        .with_local("random_seed_fast")
        .with_output(WasmType::U64),
    );
    let monotonic_clock =
        WasmExternalName::create("monotonic-clock").with_project("wasi", "clocks").with_version(version.clone());

    module.register(ImportFunction::new(monotonic_clock.clone(), "now").with_local("clock_mow").with_output(WasmType::U64));
    module.register(
        ImportFunction::new(monotonic_clock.clone(), "resolution").with_local("clock_resolution").with_output(WasmType::U64),
    );

    // wall-clock
    let wall_clock = WasmExternalName::create("wall-clock").with_project("wasi", "clocks").with_version(version.clone());
    // module.register(
    //     ImportFunction::new(wall_clock.clone(), "now").with_local("wall_clock_mow").with_output(WasmType::Structure(
    //         StructureType::new("datetime")
    //             .with_field(FieldType::new("seconds").with_type(WasmType::U64))
    //             .with_field(FieldType::new("nanoseconds").with_type(WasmType::U32)),
    //     )),
    // );

    // module.register(
    //     ImportFunction::new(
    //         WasmExternalName::create("random").with_project("wasi", "random").with_version(Version::new(0, 2, 0)),
    //         "get-random-bytes",
    //     )
    //     .with_alias("get_random_bytes")
    //     .with_input(vec![WasmParameter::new("length").with_type(WasmType::U64)])
    //     .with_output(WasmType::I32),
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
