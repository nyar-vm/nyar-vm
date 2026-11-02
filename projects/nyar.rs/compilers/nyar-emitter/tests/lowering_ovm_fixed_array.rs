//! OVM 定长数组 lowering：`ArrayFromElements` 的 `layout_id` 是前端合成身份，
//! 不保证进入 `AggregateLayoutPlan`。OVM 物理对象只有字段数、槽位无类型，因此后端
//! 必须按元素个数铸造私有布局，禁止依赖未冻结的身份，也禁止 panic 杀进程。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::NyarType;
use odin_bytecode::{decode_at, OdinHeadCode, OdinInstruction, OdinModuleData};

fn fixed_array(length: usize) -> NyarType {
    NyarType::FixedArray { element: Box::new(NyarType::Integer64 { signed: true }), length }
}

/// 单 entry block：`ArrayFromElements` 后返回 `ValueRef(0)`。
fn array_function(length: usize, elements: usize) -> ExecutableFunction {
    let array_type = fixed_array(length);
    let operands = (0..elements).map(|value| Operand::Constant(Constant::Int(value as i64))).collect();
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: array_type.clone(),
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), array_type)]),
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
                // 0 是未登记进 `AggregateLayoutPlan` 的合成身份；后端不得据此 panic。
                ..Instruction::from_kind(InstructionKind::ArrayFromElements { layout_id: 0, array_type: fixed_array(length), elements: operands })
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn lower(function: ExecutableFunction) -> nyar_emitter::Result<OdinModuleData> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("ovm_fixed_array").testing_entry_operation(main_name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

fn decoded(module: &OdinModuleData) -> Vec<OdinInstruction> {
    let mut pc = 0usize;
    let mut instructions = Vec::new();
    while pc < module.code_bytes.len() {
        let instruction = decode_at(&module.code_bytes, pc);
        assert!(instruction.size > 0, "无法解码的字节码 pc={pc}");
        pc += usize::from(instruction.size);
        instructions.push(instruction);
    }
    instructions
}

/// 正：未登记布局身份的定长数组必须按元素个数铸造物理布局并写满全部槽位。
#[test]
fn ovm_array_from_elements_with_unregistered_layout_id_lowers() {
    let module = lower(array_function(2, 2)).expect("定长数组不得因未登记 layout_id 失败");
    let instructions = decoded(&module);

    let object_new = instructions.iter().position(|instruction| instruction.code == OdinHeadCode::ObjectNew).expect("必须发射 ObjectNew");
    let layout = module.layouts.get(instructions[object_new].operand1 as usize).expect("ObjectNew 必须指向已铸造布局");
    assert_eq!(layout.field_count, 2, "布局槽位必须等于元素个数");

    let field_sets = instructions[object_new + 1..]
        .iter()
        .filter(|instruction| instruction.code == OdinHeadCode::FieldSet)
        .map(|instruction| instruction.operand1)
        .collect::<Vec<_>>();
    assert_eq!(field_sets, vec![0, 1], "元素必须逐槽写入");
}

/// 反：元素个数与冻结的定长数组长度不符时必须结构化失败，不得 panic。
#[test]
fn ovm_array_from_elements_length_mismatch_fails() {
    let error = lower(array_function(3, 2)).expect_err("元素个数与类型长度不一致必须失败");
    assert!(error.contains("元素个数与冻结的定长数组类型不符"), "错误必须点名长度合同不符: {error}");
}
