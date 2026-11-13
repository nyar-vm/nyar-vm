//! OVM `null` 常量 lowering：`Constant::Null` 必须落成 `OdinConstant::Null`，
//! 不得用 `i32 0` 冒充（可空引用 ABI 合同的正向钉死：编码/解码/VM 物化/JSON 桥均已接线）。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::NyarType;
use acorn_odin::{OdinHeadCode, OdinModuleData, decode_at};

fn nullable_slot() -> NyarType {
    NyarType::Nullable(Box::new(NyarType::Integer64 { signed: true }))
}

fn null_return_function() -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: nullable_slot(),
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), nullable_slot())]),
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
                results: vec![ValueRef(0)],
                ..Instruction::from_kind(InstructionKind::LoadConstant {
                    constant: Constant::Null,
                    ty: Some(nullable_slot()),
                })
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn lower() -> nyar_emitter::Result<OdinModuleData> {
    let name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name.clone(), null_return_function())]));
    plan.testing_module_name("ovm_null_constant").testing_entry_operation(name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

#[test]
fn ovm_null_constant_lowers_to_explicit_odin_null() {
    let module = lower().expect("null 常量必须能 lowering，不得再报缺 ABI 合同");
    let main = module.functions.iter().find(|function| function.name == "main").expect("main 必须导出");
    let mut pc = main.code_offset as usize;
    let end = pc + main.code_length as usize;
    let mut saw_null_constant = false;
    while pc < end {
        let instruction = decode_at(&module.code_bytes, pc);
        assert!(instruction.size > 0, "无法解码的字节码 pc={pc}");
        pc += usize::from(instruction.size);
        if instruction.code == OdinHeadCode::Constant
            && module.constants.get(instruction.operand1 as usize) == Some(&acorn_odin::OdinConstant::Null)
        {
            saw_null_constant = true;
        }
    }
    assert!(saw_null_constant, "返回槽必须加载显式 `OdinConstant::Null`，禁止 `i32 0` 冒充");
}
