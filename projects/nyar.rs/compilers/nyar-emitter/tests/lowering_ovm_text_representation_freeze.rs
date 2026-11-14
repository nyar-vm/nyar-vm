//! OVM 文本表示计划冻结的正反回归：每个 `utf8` SSA 槽的载体必须是发射前冻结的
//! 显式事实（raw/managed），缺载体事实是 `missing`——结构化拒收，不得解释为 raw。
//!
//! 对应 §1.29 维持拒收第 9 项第三条：删除 `ovm/mir.rs` 沿 SSA 定义链恢复文本载体
//! 并把缺布局解释为 raw 的路径；嵌套槽固定 raw 是目标合同显式声明，不是发射器推断。
//!
//! 对应 §1.30 维持拒收：程序级 raw 模式只来自正式表示计划事实
//! （[`ProgramTextRepresentation`] 显式声明并冻结进 `TargetExecutionPlan`），
//! 禁止由「布局注册表没有 `utf8` 托管布局」反推 `pure_raw`——上游漏登记整类
//! 布局时 missing 会被吸收成合法表示。正反合同：
//! - 计划显式声明 raw → 未分类文本槽按程序级事实冻结为 raw，合法发射；
//! - 布局表为空/缺登记且未声明 raw → 发射前明确报错，不得填 Raw。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, ProgramTextRepresentation, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, Value, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::{AggregateLayout, FieldLayout, LayoutId, NyarType, StorageKind, ValueOrigin};
use acorn_odin::OdinModuleData;

/// 计划冻结的 `utf8` 托管布局身份：单一 `[u8]` 槽配方（与 `Utf8Text` 同形）。
const TEXT_LAYOUT_ID: LayoutId = 11;

fn utf8_layout() -> AggregateLayout {
    AggregateLayout {
        id: TEXT_LAYOUT_ID,
        name: "Utf8Text".to_string(),
        namespace: "std.text".to_string(),
        key: None,
        storage: StorageKind::Reference,
        fields: vec![FieldLayout {
            name: "_repr".to_string(),
            ty: NyarType::Array(Box::new(NyarType::Integer8 { signed: false })),
        }],
    }
}

fn utf8() -> NyarType {
    NyarType::Utf8
}

fn i64() -> NyarType {
    NyarType::Integer64 { signed: true }
}

fn empty_function(symbol: &str, return_type: NyarType, return_layout: Option<LayoutId>, blocks: Vec<Block>) -> ExecutableFunction {
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type,
        param_types: Vec::new(),
        value_types: BTreeMap::new(),
        value_layouts: BTreeMap::new(),
        return_layout,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks,
        diagnostics: Vec::new(),
    }
}

/// 合并块参数夹具：入口块把 `utf8` 字面量经 `Jump` 实参并入块 1 的 `utf8` 块参数，
/// 块参数本身没有任何载体事实来源（无 `value_layouts` 行、非入口形参）。
fn merge_block_parameter_function() -> ExecutableFunction {
    let mut function = empty_function(
        "main",
        i64(),
        None,
        vec![
            Block {
                id: BlockRef(0),
                label: "entry".to_string(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Jump { target: BlockRef(1), arguments: vec![Operand::Constant(Constant::Utf8("abc".to_string()))] },
            },
            Block {
                id: BlockRef(1),
                label: "merge".to_string(),
                parameters: vec![ValueRef(0)],
                instructions: Vec::new(),
                terminator: Terminator::Return { value: Some(Operand::Constant(Constant::Int(7))) },
            },
        ],
    );
    function.value_types.insert(ValueRef(0), utf8());
    function
}

/// 组装计划：`managed_text` 控制是否登记 `utf8` 托管布局，`program_raw` 控制
/// 是否把程序级 raw 作为正式表示计划事实显式冻结进计划。
fn plan_for(function: ExecutableFunction, managed_text: bool, program_raw: bool) -> TargetExecutionPlan {
    let name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name.clone(), function)]));
    if managed_text {
        plan.testing_register_type_layout(utf8(), utf8_layout());
    }
    if program_raw {
        plan.freeze_program_text_representation(ProgramTextRepresentation::Raw);
    }
    plan.testing_module_name("ovm_text_representation_freeze").testing_entry_operation(name);
    plan
}

fn lower(function: ExecutableFunction, managed_text: bool, program_raw: bool) -> nyar_emitter::Result<OdinModuleData> {
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan_for(function, managed_text, program_raw)))
}

/// 反例：目标计划已登记 `utf8` 托管布局，而合并块参数没有逐槽载体事实——
/// 这是 `missing` 事实，必须在发射前结构化拒收，不得再按旧路径解释为 raw。
#[test]
fn missing_block_parameter_carrier_is_rejected_before_emission() {
    let error = lower(merge_block_parameter_function(), true, false).expect_err("缺载体事实的 `utf8` 块参数必须在发射前拒收");
    assert!(error.contains("缺少载体事实"), "错误必须是结构化的缺事实拒收，实际: {error}");
    assert!(error.contains("不得解释为 raw"), "错误必须点名禁止 raw 回落，实际: {error}");
}

/// 反例（§1.30 核心）：布局表为空且程序级表示计划没有显式声明 raw——
/// 缺登记不是「程序明确选择 raw 表示」的证据，必须在发射前明确报错，
/// 不得把未分类文本值吸收成 Raw。
#[test]
fn empty_layout_table_without_raw_fact_is_rejected_not_absorbed() {
    let error = lower(merge_block_parameter_function(), false, false).expect_err("布局表为空不得反推 raw，缺程序级显式声明必须拒收");
    assert!(error.contains("缺少载体事实"), "错误必须是结构化的缺事实拒收，实际: {error}");
    assert!(error.contains("不得解释为 raw"), "错误必须点名禁止 raw 吸收，实际: {error}");
}

/// 正例（§1.30 核心）：计划显式声明程序级 raw 表示——块参数的 raw 是正式表示
/// 计划事实沿 `TargetExecutionPlan` 传到发射期的结果，降低必须成功。
#[test]
fn explicit_raw_plan_freezes_block_parameter_as_program_fact() {
    lower(merge_block_parameter_function(), false, true).expect("显式 raw 计划事实下的 `utf8` 块参数必须按程序级事实完成降低");
}

/// 入口形参夹具：`main(text: utf8) -> i64`，槽没有任何托管布局登记，
/// raw/managed 只能由程序级表示计划事实决定。
fn utf8_entry_parameter_function() -> ExecutableFunction {
    let mut function = empty_function(
        "main",
        i64(),
        None,
        vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: vec![ValueRef(0)],
            instructions: Vec::new(),
            terminator: Terminator::Return { value: Some(Operand::Constant(Constant::Int(7))) },
        }],
    );
    function.param_types = vec![utf8()];
    function.value_types.insert(ValueRef(0), utf8());
    function.values = vec![Value { id: ValueRef(0), origin: ValueOrigin::Parameter { index: 0, name: "text".to_string() } }];
    function
}

/// 反例（§1.30）：入口形参槽解析不到托管布局而计划未显式声明 raw——
/// 规则 4 必须报 missing 分叉，不得回落成 raw 宿主串。
#[test]
fn utf8_entry_parameter_without_raw_fact_is_rejected() {
    let error = lower(utf8_entry_parameter_function(), false, false).expect_err("未声明 raw 的 `utf8` 入口形参槽必须在发射前拒收");
    assert!(error.contains("缺少 raw 事实"), "错误必须点名缺失的程序级 raw 声明，实际: {error}");
    assert!(error.contains("不得解释为 raw"), "错误必须点名禁止 raw 回落，实际: {error}");
}

/// 正例（§1.30）：程序级显式 raw 计划事实授权入口形参槽冻结为 raw，合法发射。
#[test]
fn explicit_raw_plan_freezes_utf8_entry_parameter_as_raw() {
    lower(utf8_entry_parameter_function(), false, true).expect("显式 raw 计划事实下的 `utf8` 入口形参槽必须冻结为 raw 并完成降低");
}

/// 正例：托管程序里字面量按 OVM 常量 ABI 冻结为 raw，进入托管返回槽时物化。
fn literal_to_managed_return_function() -> ExecutableFunction {
    let mut function = empty_function(
        "main",
        utf8(),
        Some(TEXT_LAYOUT_ID),
        vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![
                Instruction {
                    results: vec![ValueRef(0)],
                    ..Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Utf8("abc".to_string()), ty: Some(utf8()) })
                },
                Instruction {
                    results: vec![ValueRef(1)],
                    ..Instruction::from_kind(InstructionKind::StoreVar {
                        name: "text".to_string(),
                        value: Operand::Value(ValueRef(0)),
                        ty: Some(utf8()),
                    })
                },
            ],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
        }],
    );
    function.value_types.insert(ValueRef(0), utf8());
    function.value_types.insert(ValueRef(1), utf8());
    function
}

#[test]
fn literal_text_materializes_into_managed_return_slot() {
    let module = lower(literal_to_managed_return_function(), true, false).expect("字面量 raw 事实进入托管返回槽必须按冻结布局物化");
    assert!(
        module.imports.iter().any(|import| import.module_name == "nyar.host" && import.symbol_name == "utf8.to_bytes"),
        "raw→托管边界必须走 `utf8.to_bytes` 物化桥: {:?}",
        module.imports
    );
}

/// 正例：入口形参槽按 `frozen_entry_parameter_layouts` 派生点冻结为托管载体，
/// `Copy` 搬运继承同一事实；托管→同布局边界不发射任何转换桥。
fn managed_entry_copy_function() -> ExecutableFunction {
    let mut function = empty_function(
        "main",
        utf8(),
        Some(TEXT_LAYOUT_ID),
        vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: vec![ValueRef(0)],
            instructions: vec![Instruction {
                results: vec![ValueRef(1)],
                ..Instruction::from_kind(InstructionKind::Copy { source: Operand::Value(ValueRef(0)) })
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
        }],
    );
    function.param_types = vec![utf8()];
    function.value_types.insert(ValueRef(0), utf8());
    function.value_types.insert(ValueRef(1), utf8());
    function.values = vec![Value { id: ValueRef(0), origin: ValueOrigin::Parameter { index: 0, name: "text".to_string() } }];
    function
}

#[test]
fn managed_entry_parameter_copy_crosses_return_without_bridge() {
    let module = lower(managed_entry_copy_function(), true, false).expect("托管形参事实经搬运进入同布局返回槽必须直传");
    assert!(
        !module.imports.iter().any(|import| import.symbol_name == "utf8.to_bytes" || import.symbol_name == "utf8.from_bytes"),
        "同一冻结布局身份跨边界不得发射任何载体转换桥: {:?}",
        module.imports
    );
}
