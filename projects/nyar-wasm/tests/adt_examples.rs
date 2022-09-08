use super::*;

pub fn new_structure() -> WasmBuilder {
    let mut module = WasmBuilder::new("structure-example");
    let point = StructureItem::new("Point").with_fields(vec![
        FieldType::new("x").with_type(WasmType::F64).with_default(WasmValue::F64(0.618)),
        FieldType::new("y").with_type(WasmType::F64).with_default(WasmValue::F64(1.618)),
    ]);
    let mut point_ty: WasmType = point.clone().into();
    point_ty.set_nullable(true);

    module.insert_type(point.clone());
    module.insert_global(
        WasmVariable {
            symbol: WasmSymbol::new("point"),
            mutable: true,
            export: Default::default(),
            r#type: point_ty.clone(),
            value: point.clone().into(),
            span: Default::default(),
        }
        .with_export(true),
    );

    module.insert_function(
        FunctionType::new("Point::construct")
            .with_inputs(vec![ParameterType::new("a").with_type(WasmType::F64)])
            .with_outputs(vec![point_ty.with_nullable(true).into()])
            .with_locals(vec![
                ParameterType::new("self.x").with_type(WasmType::F64),
                ParameterType::new("self.y").with_type(WasmType::F64),
            ])
            .with_operations(vec![
                Operation::local_get("a"),
                Operation::local_set("self.x"),
                Operation::local_get("a"),
                Operation::local_set("self.y"),
                Operation::local_get("self.x"),
                Operation::local_get("self.y"),
                Operation::Construct { structure: point.symbol.clone() },
            ]),
    );
    module.insert_function(
        FunctionType::new("_start")
            .with_outputs(vec![WasmType::F64])
            .with_locals(vec![ParameterType::new("point").with_type(point.clone().into())])
            .with_operations(vec![
                Operation::CallFunction {
                    name: WasmSymbol::new("Point::construct"),
                    input: vec![Operation::Constant { value: WasmValue::F64(std::f64::consts::PI) }],
                },
                Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("y") },
                // Operation::global_set("point"),
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
    let utf32 = ArrayType::new("UTF32Text", WasmType::I32);
    module.insert_type(utf32.clone());

    module.insert_function(
        FunctionType::new("new_default").with_outputs(vec![WasmType::F32]).with_locals(vec![]).with_operations(vec![
            Operation::Default { typed: utf32.clone().into() },
            Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("x") },
        ]),
    );
    module.insert_function(
        FunctionType::new("new_valued").with_outputs(vec![WasmType::F32]).with_locals(vec![]).with_operations(vec![
            Operation::Constant { value: utf32.clone().into() },
            Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("y") },
        ]),
    );

    module.insert_function(
        FunctionType::new("_start")
            .with_outputs(vec![WasmType::F32])
            .with_locals(vec![])
            .with_operations(vec![Operation::NativeSum {
                r#type: WasmType::F32,
                terms: vec![
                    Operation::CallFunction { name: WasmSymbol::new("new_default"), input: vec![] },
                    Operation::CallFunction { name: WasmSymbol::new("new_valued"), input: vec![] },
                ],
            }])
            .with_export(true),
    );
    module
}
