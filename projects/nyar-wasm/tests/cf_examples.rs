use super::*;

pub fn fibonacci() -> WasmBuilder {
    let mut module = WasmBuilder::new("fibonacci");

    module.register_function(
        FunctionType::new("fibonacci")
            .with_inputs(vec![ParameterType::new("n").with_type(WasmType::I64)])
            .with_outputs(vec![WasmType::I64])
            .with_locals(vec![
                ParameterType::new("a").with_type(WasmType::I64),
                ParameterType::new("b").with_type(WasmType::I64),
            ])
            .with_operations(vec![])
            .with_export(true),
    );

    module.register_function(
        FunctionType::new(WasmSymbol::new("_main"))
            .with_inputs(vec![])
            .with_outputs(vec![WasmType::I64])
            .with_operations(vec![Operation::CallFunction {
                name: WasmSymbol::new("fibonacci"),
                input: vec![Operation::from(5)],
            }])
            .with_export(false),
    );
    module
}

pub fn control_flow() -> WasmBuilder {
    let mut module = WasmBuilder::new("control_flow");

    module.register_function(
        FunctionType::new(WasmSymbol::new("sum_all"))
            .with_inputs(vec![
                ParameterType::new("a").with_type(WasmType::I32),
                ParameterType::new("b").with_type(WasmType::I32),
                ParameterType::new("c").with_type(WasmType::I32),
            ])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![
                Operation::NativeSum {
                    r#type: WasmType::I32,
                    terms: vec![
                        // test_if_1(),
                        test_if_2(),
                        test_if_3(),
                        test_switch(),
                        // test_loop1(),
                    ],
                },
                Operation::Return {},
            ]),
    );

    module.register_function(
        FunctionType::new(WasmSymbol::new("_main"))
            .with_inputs(vec![])
            .with_outputs(vec![WasmType::I32])
            .with_operations(vec![Operation::CallFunction {
                name: WasmSymbol::new("sum_all"),
                input: vec![Operation::from(1), Operation::from(2), Operation::from(3)],
            }])
            .with_export(false),
    );
    module
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
                    r#type: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(1)],
                }],
                action: vec![Operation::from(2)],
            },
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    r#type: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(3)],
                }],
                action: vec![Operation::from(3)],
            },
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    r#type: WasmType::Bool,
                    codes: vec![Operation::local_get("a"), Operation::from(5)],
                }],
                action: vec![Operation::from(5)],
            },
            JumpCondition {
                condition: vec![Operation::NativeEqual {
                    r#type: WasmType::Bool,
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
