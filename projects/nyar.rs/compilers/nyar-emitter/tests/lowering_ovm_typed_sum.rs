//! OVM typed sum lowering：`SumNew / SumVariantIs / SumPayloadGet` 只消费计划冻结的
//! `sum_representations()`，物理载体固定为后端私有 `[i32 tag, payload]` 对象。
//!
//! 本测试锁定发射形状（槽 0 = tag、槽 1 = payload）与结构化失败路径：表示缺失、
//! tag 宽度越界、载荷合同不一致都不得静默回退或丢对象。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::{
    layout_choice::{SumRepresentation, SumVariantRepresentation},
    NyarType, NominalInstanceId, TypeId, VariantId,
};
use acorn_odin::{decode_at, OdinConstant, OdinHeadCode, OdinInstruction, OdinModuleData};

// 返回类型只起「非 unit」占位作用；文本 carrier 在 OVM 返回边界要求冻结的
// `return_layout`（`d3a8f0b7`），与 sum 表示无关，故用整型载体。
const SUM_TYPE: NyarType = NyarType::Integer32 { signed: true };
const I32: NyarType = NyarType::Integer32 { signed: true };

fn nominal() -> NominalInstanceId {
    NominalInstanceId::from_index(0).expect("测试 nominal 身份")
}

fn variant() -> VariantId {
    VariantId::from_index(0).expect("测试 variant 身份")
}

fn payload_type() -> TypeId {
    TypeId::from_index(0).expect("测试 payload 类型身份")
}

fn representation(nominal: NominalInstanceId, tag_width: u32, with_payload: bool) -> SumRepresentation {
    SumRepresentation {
        nominal,
        tag_width,
        variants: BTreeMap::from([(
            variant(),
            SumVariantRepresentation { id: variant(), tag: 0, payload_type: with_payload.then(payload_type) },
        )]),
    }
}

/// 单 entry block：指令序列后返回 `ValueRef(0)`。
fn sum_function(instructions: Vec<Instruction>, value_types: BTreeMap<ValueRef, NyarType>) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: SUM_TYPE,
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions,
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn sum_new(payload: Option<Operand>) -> Instruction {
    Instruction { results: vec![ValueRef(0)], ..Instruction::from_kind(InstructionKind::SumNew { nominal: nominal(), variant: variant(), payload }) }
}

fn lower(function: ExecutableFunction, representation: Option<SumRepresentation>) -> nyar_emitter::Result<OdinModuleData> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("ovm_typed_sum").testing_entry_operation(main_name);
    if let Some(representation) = representation {
        plan.testing_register_sum_representation(nominal(), representation);
    }
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

/// 登记多个 nominal 的冻结表示：每条表示只影响它自己的 nominal 身份。
fn lower_with_representations(
    function: ExecutableFunction,
    representations: Vec<SumRepresentation>,
) -> nyar_emitter::Result<OdinModuleData> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("ovm_typed_sum").testing_entry_operation(main_name);
    for representation in representations {
        plan.testing_register_sum_representation(representation.nominal, representation);
    }
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

/// 正：单 payload sum 构造 tag 与 payload 两个槽，variant 判定读取 tag 槽 0。
#[test]
fn ovm_sum_new_emits_tag_and_payload_slots() {
    let instructions = vec![
        sum_new(Some(Operand::Constant(Constant::Int(7)))),
        Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::SumVariantIs {
                nominal: nominal(),
                variant: variant(),
                object: Operand::Value(ValueRef(0)),
            })
        },
    ];
    let module = lower(
        sum_function(instructions, BTreeMap::from([(ValueRef(0), SUM_TYPE), (ValueRef(1), I32)])),
        Some(representation(nominal(), 1, true)),
    )
    .expect("冻结表示必须让 OVM sum 构造成功");

    let instructions = decoded(&module);
    let object_new = instructions.iter().position(|instruction| instruction.code == OdinHeadCode::ObjectNew).expect("必须发射 ObjectNew");
    let field_sets = instructions[object_new + 1..]
        .iter()
        .filter(|instruction| instruction.code == OdinHeadCode::FieldSet)
        .map(|instruction| instruction.operand1)
        .collect::<Vec<_>>();
    assert_eq!(field_sets, vec![0, 1], "tag 与 payload 必须各写一次槽 0 / 槽 1");
    assert!(
        instructions.iter().any(|instruction| instruction.code == OdinHeadCode::FieldGet && instruction.operand1 == 0),
        "SumVariantIs 必须读取 tag 槽 0"
    );
    assert!(module.layouts.iter().any(|layout| layout.field_count == 2), "sum 物理布局必须是两槽对象");
}

/// 正：无 payload variant 只写 tag 槽。
#[test]
fn ovm_sum_new_without_payload_emits_only_tag_slot() {
    let module = lower(
        sum_function(vec![sum_new(None)], BTreeMap::from([(ValueRef(0), SUM_TYPE)])),
        Some(representation(nominal(), 1, false)),
    )
    .expect("无 payload 的冻结变体必须构造成功");

    let instructions = decoded(&module);
    let object_new = instructions.iter().position(|instruction| instruction.code == OdinHeadCode::ObjectNew).expect("必须发射 ObjectNew");
    let field_sets = instructions[object_new + 1..]
        .iter()
        .filter(|instruction| instruction.code == OdinHeadCode::FieldSet)
        .map(|instruction| instruction.operand1)
        .collect::<Vec<_>>();
    assert_eq!(field_sets, vec![0], "无 payload 时只能写 tag 槽");
}

/// 反：未冻结该 nominal 的表示时必须结构化失败，不得静默发默认布局。
#[test]
fn ovm_sum_without_frozen_representation_fails() {
    let error = lower(sum_function(vec![sum_new(None)], BTreeMap::from([(ValueRef(0), SUM_TYPE)])), None)
        .expect_err("缺少冻结 sum 表示必须失败");
    assert!(error.contains("OVM sum 表示缺失"), "错误必须点名表示缺失: {error}");
    assert!(error.contains("nominal"), "错误必须点名 nominal: {error}");
}

/// 反：tag 宽度越界必须按 wasm 同一合同结构化失败。
#[test]
fn ovm_sum_tag_width_outside_contract_fails() {
    let error = lower(
        sum_function(vec![sum_new(None)], BTreeMap::from([(ValueRef(0), SUM_TYPE)])),
        Some(representation(nominal(), 8, false)),
    )
    .expect_err("tag_width=8 必须失败");
    assert!(error.contains("tag_width"), "错误必须点名 tag_width: {error}");
}

/// 反：计划声明 payload 但指令没有 payload 时必须失败，不得用空载荷掩盖。
#[test]
fn ovm_sum_payload_contract_mismatch_fails() {
    let error = lower(
        sum_function(vec![sum_new(None)], BTreeMap::from([(ValueRef(0), SUM_TYPE)])),
        Some(representation(nominal(), 1, true)),
    )
    .expect_err("载荷合同不一致必须失败");
    assert!(error.contains("载荷与计划合同不一致"), "错误必须点名载荷合同不一致: {error}");
}

/// 正：`SumPayloadGet` 只读取 payload 槽 1，tag 槽 0 不被复用。
#[test]
fn ovm_sum_payload_get_reads_payload_slot() {
    let instructions = vec![
        sum_new(Some(Operand::Constant(Constant::Int(7)))),
        Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::SumPayloadGet {
                nominal: nominal(),
                variant: variant(),
                object: Operand::Value(ValueRef(0)),
            })
        },
    ];
    let module = lower(
        sum_function(instructions, BTreeMap::from([(ValueRef(0), SUM_TYPE), (ValueRef(1), I32)])),
        Some(representation(nominal(), 1, true)),
    )
    .expect("带 payload 的冻结变体必须允许 SumPayloadGet");

    let instructions = decoded(&module);
    let payload_gets = instructions
        .iter()
        .filter(|instruction| instruction.code == OdinHeadCode::FieldGet && instruction.operand1 == 1)
        .count();
    assert_eq!(payload_gets, 1, "SumPayloadGet 必须且只能读取 payload 槽 1");
}

/// 反：对无 payload 的 variant 取值必须结构化失败，不得从空槽静默读出。
#[test]
fn ovm_sum_payload_get_on_unit_variant_fails() {
    let instructions = vec![
        sum_new(None),
        Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::SumPayloadGet {
                nominal: nominal(),
                variant: variant(),
                object: Operand::Value(ValueRef(0)),
            })
        },
    ];
    let error = lower(
        sum_function(instructions, BTreeMap::from([(ValueRef(0), SUM_TYPE), (ValueRef(1), I32)])),
        Some(representation(nominal(), 1, false)),
    )
    .expect_err("无 payload 的 variant 取值必须失败");
    assert!(error.contains("无 payload"), "错误必须点名无 payload 的 variant: {error}");
}

/// 正：tag 与 payload 合同按 **nominal 身份**查表，同一 `VariantId` 在不同 nominal
/// 下拥有各自的 tag；不得按 variant 短名或固定 tag 推断。
#[test]
fn ovm_sum_variant_tags_are_looked_up_per_nominal() {
    let other = NominalInstanceId::from_index(1).expect("第二个测试 nominal 身份");
    let mut other_representation = representation(other, 1, true);
    if let Some(variant_layout) = other_representation.variants.get_mut(&variant()) {
        variant_layout.tag = 2;
    }

    let instructions = vec![
        sum_new(Some(Operand::Constant(Constant::Int(7)))),
        Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::SumNew {
                nominal: other,
                variant: variant(),
                payload: Some(Operand::Constant(Constant::Int(9))),
            })
        },
        Instruction {
            results: vec![ValueRef(2)],
            ..Instruction::from_kind(InstructionKind::SumVariantIs { nominal: nominal(), variant: variant(), object: Operand::Value(ValueRef(0)) })
        },
        Instruction {
            results: vec![ValueRef(3)],
            ..Instruction::from_kind(InstructionKind::SumVariantIs { nominal: other, variant: variant(), object: Operand::Value(ValueRef(1)) })
        },
    ];
    let mut plan_representations = vec![representation(nominal(), 1, true), other_representation];
    plan_representations.sort_by_key(|entry| entry.nominal);
    let module = lower_with_representations(
        sum_function(
            instructions,
            BTreeMap::from([(ValueRef(0), SUM_TYPE), (ValueRef(1), SUM_TYPE), (ValueRef(2), I32), (ValueRef(3), I32)]),
        ),
        plan_representations,
    )
    .expect("两个 nominal 的冻结表示都必须生效");

    let tags = module
        .constants
        .iter()
        .filter_map(|constant| match constant {
            OdinConstant::Integer32(value) => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(tags.contains(&0), "nominal 0 的 tag 必须是 0: {tags:?}");
    assert!(tags.contains(&2), "nominal 1 的 tag 必须来自它自己的表示（2）: {tags:?}");
}
