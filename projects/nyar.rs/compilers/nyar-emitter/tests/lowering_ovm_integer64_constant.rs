//! OVM `i64` 常量池形状：超出 `i32` 范围必须用 `Integer64`，不得截断成 `Integer32`。
//!
//! 既有产物形状（`Integer32` + `i32_to_i64` 拓宽）只服务于 `i32` 可表示的值；
//! `value as i32` 会保留低 32 位并符号扩展，`i64::MAX` 因此被静默编成 `-1`。
//! 本文件锁定发射侧的常量种类，不依赖前端源码链路。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Operand, Terminator},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::NyarType;
use odin_bytecode::{OdinConstant, OdinImport, OdinModuleData};

fn integer64() -> NyarType {
    NyarType::Integer64 { signed: true }
}

/// 单 entry block：直接返回常量操作数（无指令）。
fn constant_function(literal: i64) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: integer64(),
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

fn lower(literal: i64) -> OdinModuleData {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), constant_function(literal))]));
    plan.testing_module_name("ovm_integer64_constant").testing_entry_operation(main_name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan)).expect("i64 常量必须能完成 OVM lowering")
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

/// `i32` 边界值仍走 `Integer32`，既有产物形状不变。
#[test]
fn i32_boundaries_stay_integer32() {
    for literal in [i64::from(i32::MAX), i64::from(i32::MIN)] {
        let module = lower(literal);
        assert_eq!(pool_integer32(&module), vec![literal as i32], "{literal} 必须走 Integer32: {:?}", module.constants);
        assert!(pool_integer64(&module).is_empty(), "{literal} 不得进 Integer64 池: {:?}", module.constants);
    }
}
