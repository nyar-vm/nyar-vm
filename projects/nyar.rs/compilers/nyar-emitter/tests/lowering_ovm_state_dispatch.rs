//! OVM `StateDispatch` 的 `case_key` 范围合同。
//!
//! `case_key` 是 `u32`，而 ovm 用 `I32Eq` 比较状态值。旧实现 `case_key as i32`
//! 会把 ≥ 2^31 的键折回负值，发出一个永远匹配不上的常量。没有任何生产者把
//! 键约束在 `i32` 内，因此超界必须在 lowering 阶段结构化失败。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, ExecutableFunction, Terminator, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::NyarType;
use odin_bytecode::{OdinConstant, OdinModuleData};

fn state_integer32() -> NyarType {
    NyarType::Integer32 { signed: true }
}

/// 两个目标块都直接 `return`，入口块按 `case_key` 派发。
fn dispatching_function(case_key: u32) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: NyarType::Unit,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), state_integer32())]),
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
        blocks: vec![
            Block {
                id: BlockRef(0),
                label: "entry".to_string(),
                parameters: vec![ValueRef(0)],
                instructions: Vec::new(),
                terminator: Terminator::StateDispatch {
                    state: ValueRef(0),
                    cases: vec![(case_key, BlockRef(1))],
                    default_target: BlockRef(2),
                },
            },
            Block { id: BlockRef(1), label: "hit".to_string(), parameters: Vec::new(), instructions: Vec::new(), terminator: Terminator::Return { value: None } },
            Block { id: BlockRef(2), label: "default".to_string(), parameters: Vec::new(), instructions: Vec::new(), terminator: Terminator::Return { value: None } },
        ],
        diagnostics: Vec::new(),
    }
}

fn lower(case_key: u32) -> nyar_emitter::Result<OdinModuleData> {
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), dispatching_function(case_key))]));
    plan.testing_module_name("ovm_state_dispatch").testing_entry_operation(main);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

/// `i32::MAX` 仍在可比较范围内，必须精确进 `Integer32` 池。
#[test]
fn case_key_at_i32_max_stays_exact() {
    let module = lower(i32::MAX as u32).expect("i32::MAX 必须能完成 ovm lowering");
    assert!(
        module.constants.iter().any(|constant| matches!(constant, OdinConstant::Integer32(value) if *value == i32::MAX)),
        "case_key 必须精确进 Integer32 池: {:?}",
        module.constants
    );
}

/// `u32::MAX` 不得被 `as i32` 折成 `-1`，必须结构化失败。
#[test]
fn case_key_beyond_i32_fails_closed() {
    let error = lower(u32::MAX).expect_err("超出 i32 的 case_key 不得静默截断");
    assert!(error.contains("case_key") && error.contains("i32"), "错误必须指明 case_key 越界，实际: {error}");
}
