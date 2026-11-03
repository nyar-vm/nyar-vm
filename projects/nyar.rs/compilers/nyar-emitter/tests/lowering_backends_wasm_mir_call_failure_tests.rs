//! WASM 静态调用计划不一致的结构化失败合同：callee 实例既没有登记为宿主
//! 导入也没有登记为本地函数时，编码期必须返回 `Err`（与 `FieldGet` 臂
//! `8050500c` 同构），禁止 panic 把可执行合同缺陷升级成编译器崩溃。
//! 同时以「已登记 wasm 宿主导入的静态调用」作正向对照，证明判定逻辑零改动。

use acorn_wasm::WasmBinaryModule;
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{ImportIndex, InstructionId, ItemInstanceId, NyarType, ProvenanceId};
use std::collections::BTreeMap;

const I32: NyarType = NyarType::Integer32 { signed: true };

/// `main()`：%0 = 静态调用 `callee`（无实参）；return %0。
fn call_main(callee: Operand) -> ExecutableFunction {
    let call = Instruction::from_kind(InstructionKind::Call { callee, arguments: Vec::new() });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![Instruction { results: vec![ValueRef(0)], ..call }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), I32)]),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![block],
        diagnostics: Vec::new(),
    }
}

fn lower(function: ExecutableFunction, import: Option<(ItemInstanceId, BackendImport)>) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("call_failure").testing_entry_operation(main_name);
    if let Some((instance, import)) = import {
        plan.testing_register_import(instance, import);
    }
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

/// fail-closed：静态调用的 callee 实例没有任何已注册 callable 条目
/// （既非导入也非本地函数）时结构化失败，禁止 panic 或按名字补造 callee。
#[test]
fn wasm_static_call_without_registered_callable_fails_structured() {
    let callee = ItemInstanceId::from_index(203).expect("测试 callee 身份");
    let error = lower(call_main(Operand::Item(callee)), None).expect_err("callee 缺少已注册 callable 条目必须返回 Err，不得 panic");
    assert!(error.contains("WASM 静态调用缺少已解析 callable identity"), "错误必须指明 callable identity 缺失: {error}");
}

/// 正向对照：callee 已按冻结身份登记为 wasm 宿主导入（`env::utf8_length`）时，
/// 静态调用照常编码为 `call` 命中导入下标，判定逻辑与结构化失败改造前一致。
#[test]
fn wasm_static_call_to_registered_host_import_lowers() {
    let callee = ItemInstanceId::from_index(203).expect("测试 callee 身份");
    let import = BackendImport {
        index: ImportIndex::from_index(0).expect("导入下标"),
        link: nyar::ExternalImportLink::host(Some(Identifier::new("wasm")), vec!["env".into(), "utf8_length".into()]),
        parameter_types: Vec::new(),
        return_type: I32,
        target_operator: None,
    };
    lower(call_main(Operand::Item(callee)), Some((callee, import))).expect("已登记宿主导入的静态调用必须正常 lower");
}

/// 宽度失配不再靠 `i64.extend_i32_s` / `i32.wrap_i64` 靠载体猜：
/// 目标槽是 i64 而实参槽已规划为 i32 时，编码期必须结构化失败，
/// 错误信息带槽类型、实际载体、操作数与来源指令。
#[test]
fn wasm_i64_slot_receiving_i32_carrier_fails_structured() {
    let callee = ItemInstanceId::from_index(203).expect("测试 callee 身份");
    let load = Instruction {
        id: InstructionId::from_index(1).expect("指令身份"),
        results: vec![ValueRef(0)],
        kind: InstructionKind::LoadConstant { constant: Constant::Int(1), ty: None },
        provenance: ProvenanceId::from_index(1).expect("provenance"),
    };
    let call = Instruction {
        id: InstructionId::from_index(2).expect("指令身份"),
        results: vec![ValueRef(1)],
        kind: InstructionKind::Call { callee: Operand::Item(callee), arguments: vec![Operand::Value(ValueRef(0))] },
        provenance: ProvenanceId::from_index(2).expect("provenance"),
    };
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![load, call],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    };
    let function = ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), I32), (ValueRef(1), I32)]),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![block],
        diagnostics: Vec::new(),
    };
    let import = BackendImport {
        index: ImportIndex::from_index(0).expect("导入下标"),
        link: nyar::ExternalImportLink::host(Some(Identifier::new("wasm")), vec!["env".into(), "utf8_length".into()]),
        parameter_types: vec![NyarType::Integer64 { signed: true }],
        return_type: I32,
        target_operator: None,
    };
    let error = lower(function, Some((callee, import))).expect_err("i32 实参送进 i64 形参槽必须结构化失败，禁止静默 extend");
    assert!(error.contains("WASM ABI 宽度失配"), "错误必须指明宽度失配: {error}");
    assert!(error.contains("slot=i64"), "错误必须带目标槽类型: {error}");
    assert!(error.contains("actual=i32"), "错误必须带实际载体: {error}");
    assert!(error.contains("origin=instruction=InstructionId(2)"), "错误必须带来源指令: {error}");
}

#[test]
fn wasm_library_export_lane_registers_host_import_callees() {
    let callee = ItemInstanceId::from_index(203).expect("测试 callee 身份");
    let import = BackendImport {
        index: ImportIndex::from_index(0).expect("导入下标"),
        link: nyar::ExternalImportLink::host(Some(Identifier::new("wasm")), vec!["env".into(), "utf8_length".into()]),
        parameter_types: Vec::new(),
        return_type: I32,
        target_operator: None,
    };
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), call_main(Operand::Item(callee)))]));
    plan.testing_module_name("library_lane").testing_entry_operation(main_name);
    plan.testing_register_import(callee, import);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "solution").expect("库导出 lane 的宿主导入 callee 必须按冻结身份注册并正常 lower");
}
