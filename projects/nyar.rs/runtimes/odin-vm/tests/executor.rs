use acorn_odin::{Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinModuleData, encode_module};
use odin_vm::{
    executor::{Executor, select_local_roots},
    jit::StackMapEntry,
    module::LoadedModule,
    *,
};

#[test]
fn runs_const_add_return_bytecode() {
    let code = vec![
        OdinHeadCode::Constant as u8,
        0,
        0,
        0,
        0,
        OdinHeadCode::Constant as u8,
        1,
        0,
        0,
        0,
        OdinHeadCode::I32Add as u8,
        OdinHeadCode::Return as u8,
    ];

    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "test".to_string(),
        constants: vec![OdinConstant::Integer32(0), OdinConstant::Integer32(1)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };

    let bytes = encode_module(&data);
    let module = LoadedModule::from_bytes(&bytes).expect("load module");
    let mut executor = Executor::new();
    let result = executor.run(&module, 0, Vec::new()).expect("execute");
    assert_eq!(result, Value::I32(1));
    assert!(executor.run(&module, 0, vec![Value::I32(2)]).is_err());
    let mut argument_module = module.clone();
    argument_module.functions[0].arity = 1;
    assert!(executor.run(&argument_module, 0, Vec::new()).is_err());
    let mut vm = odin_vm::OdinVm::new();
    assert!(vm.run(&module, "main", vec![Value::I32(2)]).is_err());
    assert!(vm.run(&argument_module, "main", Vec::new()).is_err());
}

#[test]
fn division_by_zero_traps_instead_of_returning_zero() {
    for trap_op in [OdinHeadCode::I32DivS, OdinHeadCode::I32RemS] {
        let code =
            vec![OdinHeadCode::Constant as u8, 0, 0, 0, 0, OdinHeadCode::Constant as u8, 1, 0, 0, 0, trap_op as u8, OdinHeadCode::Return as u8];
        let data = OdinModuleData {
            version: Odin_VERSION,
            name: "div_zero".to_string(),
            constants: vec![OdinConstant::Integer32(7), OdinConstant::Integer32(0)],
            functions: vec![OdinFunction {
                name: "main".to_string(),
                arity: 0,
                local_count: 0,
                code_offset: 0,
                code_length: code.len() as i32,
            }],
            imports: Vec::new(),
            exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
            witness_entries: Vec::new(),
            code_bytes: code,
            globals: Vec::new(),
            init_function_indices: Vec::new(),
            layouts: Vec::new(),
        };
        let bytes = encode_module(&data);
        let module = LoadedModule::from_bytes(&bytes).expect("load module");
        let mut executor = Executor::new();
        let result = executor.run(&module, 0, Vec::new());
        assert!(matches!(result, Err(OdinRuntimeError::DivisionByZero { .. })), "{trap_op:?} 除零必须 trap，实际 {result:?}");
    }
}

#[test]
fn runs_bitwise_ops_bytecode() {
    for (bitwise_op, lhs, rhs, expected) in [
        (OdinHeadCode::I32And, 0b1100, 0b1010, 0b1000),
        (OdinHeadCode::I32Or, 0b1100, 0b1010, 0b1110),
        (OdinHeadCode::I32Xor, 0b1100, 0b1010, 0b0110),
        (OdinHeadCode::I32Shl, 1, 4, 16),
        (OdinHeadCode::I32Shl, 1, 33, 2),
        (OdinHeadCode::I32ShrS, -16, 2, -4),
        (OdinHeadCode::I32ShrU, -16, 2, 0x3FFF_FFFC),
    ] {
        let code = vec![
            OdinHeadCode::Constant as u8,
            0,
            0,
            0,
            0,
            OdinHeadCode::Constant as u8,
            1,
            0,
            0,
            0,
            bitwise_op as u8,
            OdinHeadCode::Return as u8,
        ];
        let data = OdinModuleData {
            version: Odin_VERSION,
            name: "bitwise".to_string(),
            constants: vec![OdinConstant::Integer32(lhs), OdinConstant::Integer32(rhs)],
            functions: vec![OdinFunction {
                name: "main".to_string(),
                arity: 0,
                local_count: 0,
                code_offset: 0,
                code_length: code.len() as i32,
            }],
            imports: Vec::new(),
            exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
            witness_entries: Vec::new(),
            code_bytes: code,
            globals: Vec::new(),
            init_function_indices: Vec::new(),
            layouts: Vec::new(),
        };
        let bytes = encode_module(&data);
        let module = LoadedModule::from_bytes(&bytes).expect("load module");
        let mut executor = Executor::new();
        let result = executor.run(&module, 0, Vec::new()).expect("execute");
        assert_eq!(result, Value::I32(expected), "{bitwise_op:?}({lhs}, {rhs}) 必须等于 {expected}");
    }
}

/// 函数值 = 槽里的函数索引：`CallIndirect` 只弹 callee，实参留给新帧。
#[test]
fn runs_call_indirect_through_function_value_slot() {
    // 栈布局必须是 `[arg, callee]`：先压 7，再压 callee（函数下标 1）。
    let main_code = vec![
        OdinHeadCode::Constant as u8,
        0,
        0,
        0,
        0,
        OdinHeadCode::Constant as u8,
        1,
        0,
        0,
        0,
        OdinHeadCode::CallIndirect as u8,
        1,
        0,
        0,
        0,
        OdinHeadCode::Return as u8,
    ];
    let callee_offset = main_code.len() as i32;
    let mut code = main_code;
    code.extend_from_slice(&[
        OdinHeadCode::LoadArg as u8,
        0,
        0,
        0,
        0,
        OdinHeadCode::Constant as u8,
        2,
        0,
        0,
        0,
        OdinHeadCode::I32Mul as u8,
        OdinHeadCode::Return as u8,
    ]);

    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "call_indirect".to_string(),
        constants: vec![OdinConstant::Integer32(7), OdinConstant::Integer32(1), OdinConstant::Integer32(2)],
        functions: vec![
            OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: callee_offset },
            OdinFunction {
                name: "double".to_string(),
                arity: 1,
                local_count: 0,
                code_offset: callee_offset,
                code_length: code.len() as i32 - callee_offset,
            },
        ],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };

    let bytes = encode_module(&data);
    let module = LoadedModule::from_bytes(&bytes).expect("load module");
    let mut executor = Executor::new();
    let result = executor.run(&module, 0, Vec::new()).expect("execute");
    assert_eq!(result, Value::I32(14), "double(7) 必须经函数值间接调用返回 14");
}

/// 间接调用必须 fail-closed：callee 越界 / 非 `i32` 都是结构化错误，不许错值。
#[test]
fn call_indirect_counterexamples_fail_closed() {
    fn run_with(callee: OdinConstant, function_count: usize) -> Result<Value, OdinRuntimeError> {
        let code = vec![
            OdinHeadCode::Constant as u8,
            0,
            0,
            0,
            0,
            OdinHeadCode::Constant as u8,
            1,
            0,
            0,
            0,
            OdinHeadCode::CallIndirect as u8,
            1,
            0,
            0,
            0,
            OdinHeadCode::Return as u8,
        ];
        let data = OdinModuleData {
            version: Odin_VERSION,
            name: "call_indirect_counterexample".to_string(),
            constants: vec![OdinConstant::Integer32(7), callee],
            functions: (0..function_count)
                .map(|index| OdinFunction {
                    name: format!("f{index}"),
                    arity: 0,
                    local_count: 0,
                    code_offset: 0,
                    code_length: code.len() as i32,
                })
                .collect(),
            imports: Vec::new(),
            exports: Vec::new(),
            witness_entries: Vec::new(),
            code_bytes: code,
            globals: Vec::new(),
            init_function_indices: Vec::new(),
            layouts: Vec::new(),
        };
        let bytes = encode_module(&data);
        let module = LoadedModule::from_bytes(&bytes).expect("load module");
        let mut executor = Executor::new();
        executor.run(&module, 0, Vec::new())
    }

    // 越界索引：函数表只有 1 项。
    let out_of_range = run_with(OdinConstant::Integer32(9), 1);
    assert!(matches!(out_of_range, Err(OdinRuntimeError::FunctionIndexOutOfRange(9))), "越界函数索引必须结构化失败，实际 {out_of_range:?}");

    // `Null` callee：经槽位传递后静态类型被抹平，只能运行期拒绝。
    let null_callee = run_with(OdinConstant::Null, 2);
    assert!(
        matches!(null_callee, Err(OdinRuntimeError::TypeMismatch { expected: "function index (i32)", .. })),
        "Null callee 必须 TypeMismatch，实际 {null_callee:?}"
    );
}

#[test]
fn select_local_roots_keeps_only_mapped_slots() {
    let locals = vec![Value::I32(1), Value::I32(2), Value::I32(3)];
    let entry = StackMapEntry { instruction_index: 0, local_root_slots: vec![0, 2], operand_root_depth: None };
    let selected = select_local_roots(&locals, &entry);
    assert_eq!(selected, vec![Value::I32(1), Value::I32(3)]);
}

#[test]
fn concurrent_collect_exposes_root_handshake_on_executor() {
    let mut executor = Executor::new();
    *executor.heap_mut().policy_mut() = GcPolicy::concurrent_mark_reserved();
    let live = executor.heap_mut().alloc(ObjectPayload::LayoutObject { layout_id: 0, slots: vec![] });
    let stack = [Value::Object(live)];
    let _ = executor.collect_for_policy(GcRoots { stack: &stack, frame_locals: &[], globals: &[], frame_coroutines: &[] });
    let hs = executor.last_root_handshake();
    assert_eq!(hs.stack_slots, 1);
    assert!(hs.gray_after_roots >= 1);
    let _ = executor.last_trace_poll();
}
