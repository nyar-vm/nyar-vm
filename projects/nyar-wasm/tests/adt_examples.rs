use super::*;

pub fn new_structure() -> WasmBuilder {
    let mut module = WasmBuilder::new("example");
    let point = StructureType::new("Point").with_fields(vec![
        FieldType::new("x").with_type(WasmType::F32).with_default(WasmValue::F32(3.14)),
        FieldType::new("y").with_type(WasmType::F32).with_default(WasmValue::F32(0.618)),
    ]);
    module.insert_type(point.clone());

    module.insert_function(
        FunctionType::new("new_default").with_outputs(vec![WasmType::F32]).with_locals(vec![]).with_operations(vec![
            Operation::Default { typed: WasmType::Structure(point.clone()) },
            Operation::GetField { structure: WasmSymbol::new("Point"), field: WasmSymbol::new("x") },
        ]),
    );
    module.insert_function(
        FunctionType::new("new_valued").with_outputs(vec![WasmType::F32]).with_locals(vec![]).with_operations(vec![
            Operation::Constant { value: WasmValue::Structure(point.clone()) },
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
