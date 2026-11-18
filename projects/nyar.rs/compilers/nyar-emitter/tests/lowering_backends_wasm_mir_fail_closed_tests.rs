//! WASM MIR 后端的 fail-closed 合同：后端没有实现的指令必须在编码期返回
//! `Err`，禁止 `_ => {}` 静默跳过（目标槽保持旧值）或编成运行期 `unreachable`
//! trap（把能力缺口伪装成程序不可达）。

use nyar::Identifier;
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, Value, ValueOrigin, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldLayout, LayoutId, SemanticType, StorageKind};
use std::collections::BTreeMap;

const I32: SemanticType = SemanticType::Integer32 { signed: true };

fn function(instructions: Vec<Instruction>, value_types: BTreeMap<ValueRef, SemanticType>) -> ExecutableFunction {
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
        #[allow(deprecated)]
        blocks: vec![block],
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

/// fail-closed：WASI 进程入口形参是冻结合同——只接受「空」或「单个首位动态
/// `[utf8]` argv」。标量形参、argv 不在首位或携带额外形参等形状缺参数来源事实，
/// 写出前必须结构化拒绝，禁止按形状认 argv 或补默认值。
#[test]
fn wasi_rejects_entry_parameter_shapes_that_are_not_declared_argv() {
    let entry_with_params = |params: Vec<SemanticType>| {
        let value_types: BTreeMap<ValueRef, SemanticType> =
            params.iter().enumerate().map(|(index, ty)| (ValueRef(index as u32), ty.clone())).collect();
        let parameters: Vec<ValueRef> = (0..params.len() as u32).map(ValueRef).collect();
        let values: Vec<Value> = params
            .iter()
            .enumerate()
            .map(|(index, _)| Value { id: ValueRef(index as u32), origin: ValueOrigin::Parameter { index, name: format!("p{index}") } })
            .collect();
        ExecutableFunction {
            symbol: "main".to_string(),
            return_type: SemanticType::Unit,
            param_types: params,
            value_types,
            value_layouts: BTreeMap::new(),
            return_layout: None,
            entry: BlockRef(0),
            values,
            blocks: vec![Block {
                id: BlockRef(0),
                label: "entry".to_string(),
                parameters,
                instructions: Vec::new(),
                terminator: Terminator::Return { value: None },
            }],
        }
    };
    let argv_array = SemanticType::Array(Box::new(SemanticType::Utf8));
    for (label, params) in [("标量形参", vec![I32]), ("argv 不在首位且携带额外形参", vec![I32, argv_array])] {
        let main_name = nyar::QualifiedName::new(vec![Identifier::new("main")]);
        let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), entry_with_params(params))]));
        plan.testing_module_name("wasi_entry_contract").testing_entry_operation(main_name);
        let submission = FragmentSubmission::from_backend_plan(plan);
        let error = nyar_emitter::testing::lower_fragment_to_wasm_module(&submission, nyar::HostProjectionBoundary::WasiComponent)
            .expect_err(&format!("{label}：非法 WASI 入口合同必须结构化拒绝"));
        assert!(error.contains("进程入口合同非法"), "{label}: {error}");
    }
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
