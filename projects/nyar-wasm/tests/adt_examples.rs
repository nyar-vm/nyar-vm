use nyar_wasm::{
    ArrayType, FieldType, FunctionType, Operation, StructureItem, WasmBuilder, WasmParameter, WasmType, WasmValue,
};

pub fn new_structure() -> WasmBuilder {
    let mut module = WasmBuilder::new("structure-example");
    let point = StructureItem::new("Point").with_fields(vec![
        FieldType::new("x").with_type(WasmType::F64).with_default(WasmValue::F64(0.618)).with_readonly(),
        FieldType::new("y").with_type(WasmType::F64).with_default(WasmValue::F64(1.618)).with_readonly(),
    ]);
    let mut point_ty: WasmType = point.clone().into();
    point_ty.set_nullable(true);
    module.register(point.clone());
    // module.register(
    //     WasmVariable {
    //         symbol: WasmSymbol::new("point"),
    //         mutable: true,
    //         export: Default::default(),
    //         r#type: point_ty.clone(),
    //         value: point.clone().into(),
    //         span: Default::default(),
    //     }
    //     .with_export(true),
    // );

    module.register(
        FunctionType::new("Point::construct")
            .with_inputs(vec![WasmParameter::new("linear").with_type(WasmType::F64)])
            .with_outputs(vec![point_ty.with_nullable(false).into()])
            .with_locals(vec![
                WasmParameter::new("self.x").with_type(WasmType::F64),
                WasmParameter::new("self.y").with_type(WasmType::F64),
            ])
            .with_operations(vec![
                Operation::local_get("linear"),
                Operation::local_set("self.x"),
                Operation::local_get("linear"),
                Operation::local_set("self.y"),
                Operation::local_get("self.x"),
                Operation::local_get("self.y"),
                Operation::Construct { structure: point.symbol.clone() },
            ]),
    );
    module.register(
        FunctionType::new("_start")
            .with_outputs(vec![WasmType::Structure(point.clone().into())])
            .with_locals(vec![])
            .with_operations(vec![
                // Operation::CallFunction {
                //     name: WasmSymbol::new("Point::construct"),
                //     input: vec![Operation::Constant { value: WasmValue::F64(std::f64::consts::PI) }],
                // },
                // Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("x") },
                Operation::Default { typed: WasmType::Structure(point.into()) },
                // Operation::global_get("point"),
                // Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("y") },
                // Operation::Drop,
            ])
            .with_export(true),
    );
    module
}

pub fn new_array() -> WasmBuilder {
    let mut module = WasmBuilder::new("array-example");
    let mut utf32 = ArrayType::new("UTF32Text", WasmType::I32);
    utf32.mutable = true;

    module.register(utf32.clone());
    module.register(
        FunctionType::new("Vector::new")
            .with_inputs(vec![])
            .with_outputs(vec![WasmType::Array(Box::new(utf32.clone()))])
            .with_locals(vec![])
            .with_operations(vec![Operation::Default { typed: utf32.clone().into() }]),
    );

    // module.register(FunctionType::new("new_valued").with_outputs(vec![WasmType::F32]).with_locals(vec![]).with_operations(
    //     vec![
    //         Operation::Constant { value: utf32.clone().into() },
    //         Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("y") },
    //     ],
    // ));

    module.register(
        FunctionType::new("_start")
            .with_outputs(vec![WasmType::Array(Box::new(utf32.clone()))])
            .with_locals(vec![WasmParameter::new("array").with_type(WasmType::Array(Box::new(utf32.clone())))])
            .with_operations(vec![
                Operation::ArrayCreate {
                    r#type: utf32.clone(),
                    element: vec![
                        Operation::Constant { value: WasmValue::I32(0x48) },
                        Operation::Constant { value: WasmValue::I32(0x65) },
                        Operation::Constant { value: WasmValue::I32(0x6C) },
                        Operation::Constant { value: WasmValue::I32(0x6C) },
                        Operation::Constant { value: WasmValue::I32(0x6F) },
                    ],
                },
                Operation::ArrayFill {
                    array: vec![Operation::local_tee("array")],
                    r#type: utf32.clone(),
                    element: vec![Operation::Constant { value: WasmValue::I32(999) }],
                    start: 2,
                    length: 2,
                },
                Operation::local_get("array"),
            ])
            .with_export(true),
    );
    module
}
