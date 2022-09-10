use super::*;
use nyar_wasm::ArrayType;

pub fn new_structure() -> WasmBuilder {
    let mut module = WasmBuilder::new("structure-example");
    let point = StructureItem::new("Point").with_fields(vec![
        FieldType::new("x").with_type(WasmType::F64).with_default(WasmValue::F64(0.618)).with_readonly(),
        FieldType::new("y").with_type(WasmType::F64).with_default(WasmValue::F64(1.618)).with_readonly(),
    ]);
    let mut point_ty: WasmType = point.clone().into();
    point_ty.set_nullable(true);
    module.register_structure(&point);
    // module.insert_global(
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

    module.register_function(
        FunctionType::new("Point::construct")
            .with_inputs(vec![ParameterType::new("linear").with_type(WasmType::F64)])
            .with_outputs(vec![point_ty.with_nullable(false).into()])
            .with_locals(vec![
                ParameterType::new("self.x").with_type(WasmType::F64),
                ParameterType::new("self.y").with_type(WasmType::F64),
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
    module.register_function(
        FunctionType::new("_start")
            .with_outputs(vec![WasmType::F64])
            .with_locals(vec![])
            .with_operations(vec![
                Operation::CallFunction {
                    name: WasmSymbol::new("Point::construct"),
                    input: vec![Operation::Constant { value: WasmValue::F64(std::f64::consts::PI) }],
                },
                Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("x") },
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
    module.register_array(utf32.clone());

    module.register_function(
        FunctionType::new("Vector::new")
            .with_inputs(vec![])
            .with_outputs(vec![WasmType::Array(Box::new(utf32.clone()))])
            .with_locals(vec![])
            .with_operations(vec![Operation::Default { typed: utf32.clone().into() }]),
    );
    // module.insert_function(
    //     FunctionType::new("new_valued").with_outputs(vec![WasmType::F32]).with_locals(vec![]).with_operations(vec![
    //         Operation::Constant { value: utf32.clone().into() },
    //         Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("y") },
    //     ]),
    // );

    module.register_function(
        FunctionType::new("_start")
            .with_outputs(vec![WasmType::Array(Box::new(utf32))])
            .with_locals(vec![])
            .with_operations(vec![Operation::CallFunction { name: WasmSymbol::new("Vector::new"), input: vec![] }])
            .with_export(true),
    );
    module
}
