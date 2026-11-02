//! OVM `PerformEffect { Raise }` 的 resume 参数落槽合同。
//!
//! 前端对未捕获 `raise` 生成的 resume 块携带一个 `Resume = !` 形参：VM 在 handler
//! resume 时把 resume 值留在栈顶，后端必须把这个值直接落到 resume 块的形参槽，
//! 不得按零实参跳转把形参留在未定义状态。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, EffectKind, ExecutableFunction, Terminator, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::NyarType;
use odin_bytecode::{OdinHeadCode, OdinModuleData, decode_at};

/// `main() -> void`：入口块 `PerformEffect Raise`，resume 块带一个 `Bottom` 形参后 `return`。
fn raising_function() -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: NyarType::Unit,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), NyarType::Bottom)]),
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
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::PerformEffect { effect: EffectKind::Raise, payload: None, resume_target: BlockRef(1) },
            },
            Block {
                id: BlockRef(1),
                label: "raise_resume".to_string(),
                parameters: vec![ValueRef(0)],
                instructions: Vec::new(),
                terminator: Terminator::Return { value: None },
            },
        ],
        diagnostics: Vec::new(),
    }
}

fn lower(function: ExecutableFunction) -> nyar_emitter::Result<OdinModuleData> {
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), function)]));
    plan.testing_module_name("ovm_raise_resume").testing_entry_operation(main);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

/// 正：`raise` 与 resume 块之间必须有一条 `StoreLocal`，把 VM 留在栈顶的 resume 值
/// 落入 resume 形参槽；`PerformEffect` 之后不得直接 `Jump` 而跳过落槽。
#[test]
fn ovm_raise_stores_resume_value_into_block_parameter() {
    let module = lower(raising_function()).expect("带 resume 形参的 `Raise` 必须完成 ovm lowering");
    let mut pc = 0usize;
    let mut instructions = Vec::new();
    while pc < module.code_bytes.len() {
        let instruction = decode_at(&module.code_bytes, pc);
        assert!(instruction.size > 0, "无法解码的字节码 pc={pc}");
        pc += usize::from(instruction.size);
        instructions.push(instruction);
    }
    let perform = instructions.iter().position(|instruction| instruction.code == OdinHeadCode::PerformEffect).expect("必须发射 PerformEffect");
    let next = &instructions[perform + 1];
    assert_eq!(next.code, OdinHeadCode::StoreLocal, "PerformEffect 之后必须先把 resume 值落槽，实际下一条为 {:?}", next.code);
}
