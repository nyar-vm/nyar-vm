//! WASM MIR 后端的 fail-closed 合同：后端没有实现的指令必须在编码期返回
//! `Err`，禁止 `_ => {}` 静默跳过（目标槽保持旧值）或编成运行期 `unreachable`
//! trap（把能力缺口伪装成程序不可达）。

use nyar::{Identifier, NamePath};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldLayout, LayoutId, NyarType, StorageKind};
use std::collections::BTreeMap;

const I32: NyarType = NyarType::Integer32 { signed: true };

fn function(instructions: Vec<Instruction>, value_types: BTreeMap<ValueRef, NyarType>) -> ExecutableFunction {
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions,
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: Vec::new(),
        value_types,
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

fn lower(function: ExecutableFunction, layouts: Option<AggregateLayoutPlan>) -> nyar_emitter::Result<acorn_wasm::WasmBinaryModule> {
    let main_name = nyar::QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("wasm_fail_closed").testing_entry_operation(main_name);
    if let Some(layouts) = layouts {
        plan.testing_aggregate_layouts(layouts);
    }
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
}

/// fail-closed：`PatternMatch` 没有 wasm lowering 时必须编译期报错，
/// 禁止 `encode_unreachable` 把未实现指令编成运行期 trap。lowering 入口先用
/// pattern-matching 合同拒绝残差指令，`emit_instruction` 的 `PatternMatch` 臂
/// 只是 release 构建下的纵深防御。
#[test]
fn wasm_pattern_match_fails_structured_instead_of_unreachable() {
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(1), ty: Some(I32) });
    let pattern =
        Instruction::from_kind(InstructionKind::PatternMatch { value: Operand::Value(ValueRef(0)), pattern_debug: "fixture".to_string() });
    let error = lower(function(vec![Instruction { results: vec![ValueRef(0)], ..load }, pattern], BTreeMap::from([(ValueRef(0), I32)])), None)
        .expect_err("未实现的 PatternMatch 必须返回 Err，不得编成 unreachable trap");
    assert!(error.contains("pattern matching contract violation"), "错误必须来自编译期合同而不是运行期 trap: {error}");
    assert!(error.contains("ResidualPatternMatchInstruction"), "错误必须指明残差 PatternMatch 指令: {error}");
}

/// fail-closed：兜底臂不再是 `_ => {}`；未覆盖的指令种类（`LoadSymbol`）同样结构化失败。
#[test]
fn wasm_unhandled_instruction_kind_fails_structured() {
    let symbol = Instruction::from_kind(InstructionKind::LoadSymbol {
        path: NamePath::new(vec![Identifier::new("fixture"), Identifier::new("answer")]),
    });
    let error = lower(function(vec![Instruction { results: vec![ValueRef(0)], ..symbol }], BTreeMap::from([(ValueRef(0), I32)])), None)
        .expect_err("兜底臂必须 fail-closed，不得静默跳过指令");
    assert!(error.contains("未实现指令 LoadSymbol"), "错误必须指明被吞掉的指令种类: {error}");
}

/// fail-closed：非空值聚合的 `AggregateCopy` 缺地址槽时不能静默跳过
/// （静默会让目标槽保持旧值），必须结构化失败。
#[test]
fn wasm_aggregate_copy_without_address_slot_fails_structured() {
    let layout_id: LayoutId = 0;
    let layout = AggregateLayout {
        id: layout_id,
        name: "FixtureAggregate".to_string(),
        namespace: String::new(),
        key: None,
        storage: StorageKind::Value,
        fields: vec![FieldLayout { name: "payload".to_string(), ty: I32 }],
    };
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(1), ty: Some(I32) });
    let copy = Instruction::from_kind(InstructionKind::AggregateCopy {
        layout_id,
        source: Operand::Constant(Constant::Int(1)),
        dest: Operand::Value(ValueRef(0)),
    });
    let error = lower(
        function(vec![Instruction { results: vec![ValueRef(0)], ..load }, copy], BTreeMap::from([(ValueRef(0), I32)])),
        Some(AggregateLayoutPlan { layouts: vec![layout], ..Default::default() }),
    )
    .expect_err("非地址型 AggregateCopy 缺地址槽必须返回 Err，不得静默跳过");
    assert!(error.contains("非地址型聚合缺少地址槽"), "错误必须指明 AggregateCopy 地址槽缺失: {error}");
}

/// fail-closed：`AggregateCopy` 缺 layout 时不能编 `unreachable` 后返回成功
/// （把计划缺事实伪装成运行期 trap），必须发射前结构化拒绝并带值/槽坐标。
#[test]
fn wasm_aggregate_copy_without_layout_fails_structured() {
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(1), ty: Some(I32) });
    let copy = Instruction::from_kind(InstructionKind::AggregateCopy {
        layout_id: 7,
        source: Operand::Value(ValueRef(0)),
        dest: Operand::Value(ValueRef(0)),
    });
    // layouts 传 None：layout_id=7 未登记，缺 layout 属缺事实。
    let error = lower(function(vec![Instruction { results: vec![ValueRef(0)], ..load }, copy], BTreeMap::from([(ValueRef(0), I32)])), None)
        .expect_err("缺 layout 的 AggregateCopy 必须返回 Err，不得编 unreachable 伪装成功");
    assert!(error.contains("AggregateCopy 缺少 layout 身份"), "错误必须指明 layout 缺失: {error}");
    assert!(error.contains("source=") && error.contains("dest="), "错误必须携带源/目标坐标: {error}");
}

/// fail-closed：gc struct 深拷贝路径缺引用槽时不能静默返回成功，
/// 必须结构化拒绝并点名缺失的 dest/source 槽。
#[test]
fn wasm_aggregate_copy_gc_missing_reference_slot_fails_structured() {
    let layout_id: LayoutId = 0;
    let layout = AggregateLayout {
        id: layout_id,
        name: "FixtureRef".to_string(),
        namespace: String::new(),
        key: None,
        storage: StorageKind::Reference,
        fields: vec![FieldLayout { name: "payload".to_string(), ty: I32 }],
    };
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(1), ty: Some(I32) });
    // dest 是 I32 标量槽（非引用），storage=Reference 触发 gc struct 分支 → 缺 dest 引用槽。
    let copy = Instruction::from_kind(InstructionKind::AggregateCopy {
        layout_id,
        source: Operand::Value(ValueRef(0)),
        dest: Operand::Value(ValueRef(0)),
    });
    let error = lower(
        function(vec![Instruction { results: vec![ValueRef(0)], ..load }, copy], BTreeMap::from([(ValueRef(0), I32)])),
        Some(AggregateLayoutPlan { layouts: vec![layout], ..Default::default() }),
    )
    .expect_err("gc struct 分支缺引用槽必须返回 Err，不得静默返回成功");
    assert!(error.contains("gc struct 分支缺少 dest 引用槽"), "错误必须指明 dest 引用槽缺失: {error}");
}

/// fail-closed：零字段值布局缺 dest 地址槽时不能静默返回成功。
#[test]
fn wasm_aggregate_copy_zero_field_missing_dest_slot_fails_structured() {
    let layout_id: LayoutId = 0;
    let layout = AggregateLayout {
        id: layout_id,
        name: "FixtureEmpty".to_string(),
        namespace: String::new(),
        key: None,
        storage: StorageKind::Value,
        fields: Vec::new(),
    };
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(1), ty: Some(I32) });
    // dest 是常量 → operand_address_local 返回 None → 缺 dest 地址槽。
    let copy = Instruction::from_kind(InstructionKind::AggregateCopy {
        layout_id,
        source: Operand::Constant(Constant::Int(2)),
        dest: Operand::Constant(Constant::Int(3)),
    });
    let error = lower(
        function(vec![Instruction { results: vec![ValueRef(0)], ..load }, copy], BTreeMap::from([(ValueRef(0), I32)])),
        Some(AggregateLayoutPlan { layouts: vec![layout], ..Default::default() }),
    )
    .expect_err("零字段值布局缺 dest 地址槽必须返回 Err，不得静默返回成功");
    assert!(error.contains("零字段值布局缺少 dest 地址槽"), "错误必须指明 dest 地址槽缺失: {error}");
}
