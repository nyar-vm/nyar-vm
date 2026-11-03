//! OVM `i64` 常量池形状：超出 `i32` 范围必须用 `Integer64`，不得截断成 `Integer32`。
//!
//! 既有产物形状（`Integer32` + `i32_to_i64` 拓宽）只服务于 `i32` 可表示的值；
//! `value as i32` 会保留低 32 位并符号扩展，`i64::MAX` 因此被静默编成 `-1`。
//! 本文件锁定发射侧的常量种类，不依赖前端源码链路。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{ExternalImportLink, Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    contracts::{
        Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef,
    },
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::{ImportIndex, InstructionId, ItemInstanceId, NyarType, ProvenanceId};
use odin_bytecode::{OdinConstant, OdinImport, OdinModuleData};

fn integer64() -> NyarType {
    NyarType::Integer64 { signed: true }
}

/// 单 entry block：直接返回常量操作数（无指令）。
fn constant_function(literal: i64, return_type: NyarType) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type,
        param_types: Vec::new(),
        value_types: BTreeMap::new(),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: Some(Operand::Constant(Constant::Int(literal))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn try_lower(literal: i64, return_type: NyarType) -> Result<OdinModuleData, String> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), constant_function(literal, return_type))]));
    plan.testing_module_name("ovm_integer64_constant").testing_entry_operation(main_name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

fn lower(literal: i64) -> OdinModuleData {
    try_lower(literal, integer64()).expect("i64 常量必须能完成 OVM lowering")
}

fn lower_with_return_type(literal: i64, return_type: NyarType) -> OdinModuleData {
    try_lower(literal, return_type).expect("槽类型内可表示的常量必须能完成 OVM lowering")
}

fn pool_integer32(module: &OdinModuleData) -> Vec<i32> {
    module
        .constants
        .iter()
        .filter_map(|constant| match constant {
            OdinConstant::Integer32(value) => Some(*value),
            _ => None,
        })
        .collect()
}

fn pool_integer64(module: &OdinModuleData) -> Vec<i64> {
    module
        .constants
        .iter()
        .filter_map(|constant| match constant {
            OdinConstant::Integer64(value) => Some(*value),
            _ => None,
        })
        .collect()
}

fn widens_through_host_import(module: &OdinModuleData) -> bool {
    module.imports.iter().any(|OdinImport { module_name, symbol_name, .. }| module_name == "nyar.host" && symbol_name == "i32_to_i64")
}

/// `i64::MAX` 必须以 `Integer64` 进池；若走 `value as i32` 会得到 `Integer32(-1)`。
#[test]
fn i64_max_uses_integer64_pool_entry() {
    let module = lower(i64::MAX);
    assert_eq!(pool_integer64(&module), vec![i64::MAX], "i64::MAX 必须原样进 Integer64 池: {:?}", module.constants);
    // 截断对照：旧实现会把 `i64::MAX` 编成 `Integer32(-1)`，再靠 `i32_to_i64` 符号扩展。
    assert!(pool_integer32(&module).iter().all(|value| *value != -1), "常量池不得出现截断产物 -1: {:?}", module.constants);
    assert!(!widens_through_host_import(&module), "Integer64 常量不需要 `i32_to_i64` 拓宽: {:?}", module.imports);
}

/// `i64::MIN` 同理，必须原样进 `Integer64`。
#[test]
fn i64_min_uses_integer64_pool_entry() {
    let module = lower(i64::MIN);
    assert_eq!(pool_integer64(&module), vec![i64::MIN], "i64::MIN 必须原样进 Integer64 池: {:?}", module.constants);
    assert!(pool_integer32(&module).is_empty(), "i64::MIN 不得退化成 Integer32: {:?}", module.constants);
}

/// 刚超 `i32` 一步的 `2147483648` 必须精确，不能回绕成 `-2147483648`。
#[test]
fn just_above_i32_max_uses_integer64_pool_entry() {
    let module = lower(2_147_483_648);
    assert_eq!(pool_integer64(&module), vec![2_147_483_648], "2147483648 必须精确进 Integer64 池: {:?}", module.constants);
    assert!(pool_integer32(&module).is_empty(), "2147483648 不得回绕成 Integer32: {:?}", module.constants);
}

/// `i32` 边界值落在 `i64` 槽：载体仍是 `Integer32` 池 + `i32_to_i64` 拓宽（编码选择），
/// 但决定「要不要拓宽」的是槽类型，不是字面量大小。
#[test]
fn i64_slot_small_value_widens_by_slot_type() {
    let module = lower(i64::from(i32::MAX));
    assert_eq!(pool_integer32(&module), vec![i32::MAX], "小值以 Integer32 进池: {:?}", module.constants);
    assert!(pool_integer64(&module).is_empty(), "小值不需要 Integer64 池条目: {:?}", module.constants);
    assert!(widens_through_host_import(&module), "i64 槽必须按 i64 载体拓宽: {:?}", module.imports);
}

/// 同样大小的字面量落在 `Integer32` 槽：不得拓宽，也不得生成 i64 载体。
#[test]
fn i32_slot_small_value_does_not_widen() {
    let module = lower_with_return_type(i64::from(i32::MAX), NyarType::Integer32 { signed: true });
    assert_eq!(pool_integer32(&module), vec![i32::MAX], "i32 槽常量必须直接进 Integer32 池: {:?}", module.constants);
    assert!(pool_integer64(&module).is_empty(), "i32 槽不得产生 Integer64 池条目: {:?}", module.constants);
    assert!(!widens_through_host_import(&module), "i32 槽不得拓宽: {:?}", module.imports);
}

/// 槽已规划为 i32 但常量越界：结构化失败，禁止静默回绕。
#[test]
fn i32_slot_constant_out_of_range_fails_structured() {
    let error = try_lower(2_147_483_648, NyarType::Integer32 { signed: true })
        .expect_err("i32 槽装不下 2147483648，必须结构化失败");
    assert!(error.contains("超出目标 Integer32 槽的表示范围"), "错误必须指明槽类型与越界值: {error}");
}

/// 整数常量送进非整数槽：结构化失败，禁止把整数载荷塞进 Boolean/Utf8 槽。
#[test]
fn integer_constant_in_non_integer_slot_fails_structured() {
    let error = try_lower(7, NyarType::Boolean).expect_err("整数常量不得落进 Boolean 槽");
    assert!(error.contains("与已规划槽类型不匹配"), "错误必须指明槽类型不匹配: {error}");
}

/// `LoadConstant` 指令的位宽同样必须来自结果槽，而不是字面量大小。
#[test]
fn load_constant_result_slot_decides_width() {
    for (slot_type, expect_widen) in [(integer64(), true), (NyarType::Integer32 { signed: true }, false)] {
        let module = try_lower_load_constant(7, slot_type.clone()).expect("槽可表示的常量必须能 lowering");
        assert_eq!(pool_integer32(&module), vec![7], "7 必须进 Integer32 池: {:?}", module.constants);
        assert_eq!(widens_through_host_import(&module), expect_widen, "槽 {slot_type} 的拓宽决策错误: {:?}", module.imports);
    }
}

fn try_lower_load_constant(literal: i64, slot_type: NyarType) -> Result<OdinModuleData, String> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let function = ExecutableFunction {
        symbol: "main".to_string(),
        return_type: slot_type.clone(),
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), slot_type)]),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(0)],
                kind: InstructionKind::LoadConstant { constant: Constant::Int(literal), ty: None },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    };
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("ovm_integer64_constant").testing_entry_operation(main_name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

fn integer32() -> NyarType {
    NyarType::Integer32 { signed: true }
}

/// `echo(a: i32) -> i32`：入口形参即返回值。
fn identity_i32(symbol: &str) -> ExecutableFunction {
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type: integer32(),
        param_types: vec![integer32()],
        value_types: BTreeMap::from([(ValueRef(0), integer32())]),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: vec![ValueRef(0)],
            instructions: Vec::new(),
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

/// `main() -> i32`：把整型字面量作为 i32 形参传给 `echo` 并返回结果。
fn call_with_i32_constant(symbol: &str, callee: ItemInstanceId, literal: i64) -> ExecutableFunction {
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type: integer32(),
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), integer32())]),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(0)],
                kind: InstructionKind::Call { callee: Operand::Item(callee), arguments: vec![Operand::Constant(Constant::Int(literal))] },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn try_lower_i32_call(literal: i64) -> Result<OdinModuleData, String> {
    let callee = ItemInstanceId::from_index(0).expect("echo 实例身份");
    let echo_name = QualifiedName::new(vec![Identifier::new("echo")]);
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([
        (echo_name, identity_i32("echo")),
        (main_name.clone(), call_with_i32_constant("main", callee, literal)),
    ]));
    plan.testing_module_name("ovm_integer64_constant").testing_entry_operation(main_name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

/// i32 形参 + 可表示常量实参：必须按 i32 槽位发 `Integer32`，不得静默拓宽。
#[test]
fn i32_parameter_constant_in_range_matches_slot() {
    let module = try_lower_i32_call(7).expect("7 在 i32 形参范围内，必须能 lowering");
    assert_eq!(pool_integer32(&module), vec![7], "常量实参必须按 i32 形参槽进 Integer32 池: {:?}", module.constants);
    assert!(pool_integer64(&module).is_empty(), "i32 形参槽不得出现 Integer64 池条目: {:?}", module.constants);
    assert!(!widens_through_host_import(&module), "i32 形参槽不得拓宽: {:?}", module.imports);
}

/// i32 形参 + 越界常量实参：必须结构化失败，禁止静默截断/拓宽。
#[test]
fn i32_parameter_constant_out_of_range_fails_structured() {
    let error = try_lower_i32_call(2_147_483_648).expect_err("i32 形参装不下 2147483648，必须结构化失败");
    assert!(error.contains("超出目标 Integer32 槽的表示范围"), "错误必须指明 i32 形参槽越界: {error}");
}

/// `nyar.host` 边界 + `ovm` 平台标签 + 单段符号的外部导入（测试专用）。
fn host_echo_import(symbol: &str, parameter_types: Vec<NyarType>) -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("导入下标"),
        link: ExternalImportLink::host(Some(Identifier::new("ovm")), vec![symbol.to_string()]),
        parameter_types,
        return_type: NyarType::Integer32 { signed: true },
        target_operator: None,
    }
}

/// `main() -> i32`：把一个整型字面量作为外部调用实参并返回调用结果。
fn call_host_with_constant(symbol: &str, callee: ItemInstanceId, literal: i64) -> ExecutableFunction {
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type: NyarType::Integer32 { signed: true },
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), NyarType::Integer32 { signed: true })]),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(0)],
                kind: InstructionKind::Call { callee: Operand::Item(callee), arguments: vec![Operand::Constant(Constant::Int(literal))] },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn try_lower_host_call(symbol: &str, parameter_types: Vec<NyarType>, literal: i64) -> Result<OdinModuleData, String> {
    // `main` 占函数实例 0；导入用独立身份，避免与本地函数混淆。
    let callee = ItemInstanceId::from_index(203).expect("导入实例身份");
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        main_name.clone(),
        call_host_with_constant("main", callee, literal),
    )]));
    plan.testing_register_import(callee, host_echo_import(symbol, parameter_types));
    plan.testing_module_name("ovm_integer64_constant").testing_entry_operation(main_name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

/// 外部调用形参声明 `Integer32` + 可表示常量：按 Integer32 发射，不得拓宽。
#[test]
fn host_i32_parameter_constant_matches_signature() {
    let module = try_lower_host_call("echo_i32", vec![NyarType::Integer32 { signed: true }], 7)
        .expect("7 在 i32 外部形参范围内，必须能 lowering");
    assert_eq!(pool_integer32(&module), vec![7], "外部实参必须按签名进 Integer32 池: {:?}", module.constants);
    assert!(pool_integer64(&module).is_empty(), "i32 外部形参不得出现 Integer64 池条目: {:?}", module.constants);
    assert!(!widens_through_host_import(&module), "i32 外部形参不得拓宽: {:?}", module.imports);
    assert!(
        module.imports.iter().any(|OdinImport { module_name, symbol_name, .. }| module_name == "nyar.host" && symbol_name == "echo_i32"),
        "外部调用必须命中导入符号: {:?}",
        module.imports
    );
}

/// 外部调用形参声明 `Integer64` + 超出 i32 的常量：按 Integer64 原样发射，不得截断。
#[test]
fn host_i64_parameter_constant_keeps_width() {
    let module = try_lower_host_call("echo_i64", vec![integer64()], 2_147_483_648).expect("i64 外部形参必须能 lowering");
    assert_eq!(pool_integer64(&module), vec![2_147_483_648], "2147483648 必须原样进 Integer64 池: {:?}", module.constants);
    assert!(pool_integer32(&module).is_empty(), "i64 外部实参不得回绕成 Integer32: {:?}", module.constants);
    assert!(
        module.imports.iter().any(|OdinImport { module_name, symbol_name, .. }| module_name == "nyar.host" && symbol_name == "echo_i64"),
        "外部调用必须命中导入符号: {:?}",
        module.imports
    );
}

/// 外部调用形参声明 `Integer32` 但常量越界：结构化失败，错误带 import/实参序号/期望与实际类型。
#[test]
fn host_i32_parameter_out_of_range_fails_structured() {
    let error = try_lower_host_call("echo_i32", vec![NyarType::Integer32 { signed: true }], 2_147_483_648)
        .expect_err("i32 外部形参装不下 2147483648，必须结构化失败");
    assert!(error.contains("OVM 外部调用"), "错误必须指明外部调用: {error}");
    assert!(error.contains("echo_i32"), "错误必须带 import 符号: {error}");
    assert!(error.contains("#0"), "错误必须带实参序号: {error}");
    assert!(error.contains("期望类型=i32"), "错误必须带期望类型: {error}");
    assert!(error.contains("实际类型=Int(2147483648)"), "错误必须带实际字面量: {error}");
    assert!(error.contains("2147483648"), "错误必须带越界常量值: {error}");
}

/// 外部导入未声明该参数的签名类型：结构化失败，不得回落到 `Integer64` 默认。
#[test]
fn host_parameter_signature_missing_fails_structured() {
    let error = try_lower_host_call("echo_i32", Vec::new(), 7).expect_err("签名缺项必须结构化失败");
    assert!(error.contains("缺少已规划签名类型"), "错误必须指明签名缺失: {error}");
    assert!(error.contains("echo_i32"), "错误必须带 import 符号: {error}");
    assert!(error.contains("#0"), "错误必须带实参序号: {error}");
    assert!(error.contains("禁止回落到 semantic MIR 的 `Integer64` 默认"), "错误必须声明禁止回落: {error}");
}
