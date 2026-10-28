use std::collections::BTreeMap;

use nyar_types::{
    CanonicalArrayInitialization, CanonicalBlock, CanonicalBlockId, CanonicalCallee, CanonicalConstant, CanonicalFunction,
    CanonicalInstruction, CanonicalOperation, CanonicalTerminator, CanonicalWitness, FieldId, FieldRecord, ItemInstanceId, MirValueId,
    NominalInstanceId, StructuredDiagnosticSet, TypeId, canonical_program::CanonicalEffectKind,
};

use crate::compiler::{
    mir::{MirConstant, MirFunction, MirOperand, MirOperation, MirTerminator, MirValueOrigin},
    types::hir::ValkyrieType,
};

use super::errors::{error_without_module, type_id};

pub(crate) fn lower_function(
    function: &MirFunction,
    instance: ItemInstanceId,
    ids: &BTreeMap<ValkyrieType, TypeId>,
    nominal_by_type: &BTreeMap<TypeId, NominalInstanceId>,
    fields: &BTreeMap<FieldId, FieldRecord>,
    field_by_owner: &BTreeMap<(NominalInstanceId, FieldId), FieldId>,
    field_names: &BTreeMap<FieldId, String>,
    next_instruction: &mut u32,
) -> Result<CanonicalFunction, StructuredDiagnosticSet> {
    let value_types = function
        .value_types
        .iter()
        .map(|(value, ty)| {
            Ok((MirValueId::from_index(value.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出"))?, type_id(ids, ty)?))
        })
        .collect::<Result<BTreeMap<_, _>, StructuredDiagnosticSet>>()?;
    let (parameters, parameter_names) = canonical_entry_parameters(function, ids)?;
    let blocks = function
        .blocks
        .iter()
        .map(|block| {
            let id = CanonicalBlockId(block.id.0);
            let parameters = block
                .parameters
                .iter()
                .map(|value| {
                    let value = MirValueId::from_index(value.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出"))?;
                    Ok((value, *value_types.get(&value).ok_or_else(|| error_without_module("CAN005", "块参数缺少类型事实"))?))
                })
                .collect::<Result<_, StructuredDiagnosticSet>>()?;
            let instructions = block
                .instructions
                .iter()
                .map(|instruction| {
                    let id = nyar_types::InstructionId::from_index(*next_instruction)
                        .ok_or_else(|| error_without_module("CAN016", "instruction identity 溢出"))?;
                    *next_instruction =
                        (*next_instruction).checked_add(1).ok_or_else(|| error_without_module("CAN016", "instruction identity 溢出"))?;
                    Ok(CanonicalInstruction {
                        id,
                        results: instruction
                            .results
                            .iter()
                            .map(|value| {
                                MirValueId::from_index(value.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出"))
                            })
                            .collect::<Result<_, _>>()?,
                        operation: lower_operation(
                            &instruction.kind,
                            &instruction.results,
                            &function.value_types,
                            ids,
                            nominal_by_type,
                            fields,
                            field_by_owner,
                            field_names,
                        )?,
                    })
                })
                .collect::<Result<_, StructuredDiagnosticSet>>()?;
            Ok((id, CanonicalBlock { id, parameters, instructions, terminator: lower_terminator(&block.terminator)? }))
        })
        .collect::<Result<BTreeMap<_, _>, StructuredDiagnosticSet>>()?;
    Ok(CanonicalFunction {
        instance,
        parameters,
        parameter_names,
        return_type: type_id(ids, &function.return_type)?,
        value_types,
        entry: CanonicalBlockId(function.entry.0),
        blocks,
    })
}

/// 收集入口参数的 SSA 值与 canonical 参数名。
///
/// 参数名必须来自 MIR 的 `MirValueOrigin::Parameter { index, name }`（compiler
/// schema 的声明名），并按 index 排成声明顺序；index 重复、越界、数量不一致
/// 都在此按 `CAN032` 失败。
fn canonical_entry_parameters(
    function: &MirFunction,
    ids: &BTreeMap<ValkyrieType, TypeId>,
) -> Result<(Vec<(MirValueId, TypeId)>, Vec<String>), StructuredDiagnosticSet> {
    let invalid = || error_without_module("CAN032", format!("函数 `{}` 的声明参数、SSA 参数来源与入口块参数不一致", function.display_symbol));
    let entry = function.blocks.iter().find(|block| block.id == function.entry).ok_or_else(invalid)?;
    let mut origins = BTreeMap::new();
    for value in &function.values {
        if let MirValueOrigin::Parameter { index, name } = &value.origin {
            if function.param_types.get(*index).is_none() || origins.insert(*index, (value.id, name.clone())).is_some() {
                return Err(invalid());
            }
        }
    }
    if origins.len() != function.param_types.len() || entry.parameters.len() != function.param_types.len() {
        return Err(invalid());
    }
    let mut parameters = Vec::with_capacity(function.param_types.len());
    let mut parameter_names = Vec::with_capacity(function.param_types.len());
    for (index, ty) in function.param_types.iter().enumerate() {
        let (value, name) = origins.get(&index).ok_or_else(invalid)?;
        if entry.parameters.get(index) != Some(value) || function.value_types.get(value) != Some(ty) {
            return Err(invalid());
        }
        parameters.push((
            MirValueId::from_index(value.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出"))?,
            type_id(ids, ty)?,
        ));
        parameter_names.push(name.clone());
    }
    Ok((parameters, parameter_names))
}

fn resolve_sum_nominal(
    mir_nominal: NominalInstanceId,
    shape: Option<&ValkyrieType>,
    ids: &BTreeMap<ValkyrieType, TypeId>,
    sum_instance_by_type: &BTreeMap<TypeId, NominalInstanceId>,
) -> Result<NominalInstanceId, StructuredDiagnosticSet> {
    // 不得用 MIR 原始 nominal 兜底；必须有具体类型形状和已映射的泛型实例。
    let ty = shape.ok_or_else(|| error_without_module("CAN056", format!("sum nominal {mir_nominal:?} 缺少具体类型形状，无法解析")))?;
    let concrete = type_id(ids, ty)?;
    sum_instance_by_type
        .get(&concrete)
        .copied()
        .ok_or_else(|| error_without_module("CAN056", format!("sum 具体类型 {concrete:?} 缺少 NominalInstanceId 映射")))
}

/// 按静态类型形状解析聚合实例 nominal；不得回落到 MIR 声明 nominal。
fn resolve_aggregate_nominal(
    shape: Option<&ValkyrieType>,
    ids: &BTreeMap<ValkyrieType, TypeId>,
    nominal_by_type: &BTreeMap<TypeId, NominalInstanceId>,
) -> Result<NominalInstanceId, StructuredDiagnosticSet> {
    let ty = shape.ok_or_else(|| error_without_module("CAN058", "聚合操作缺少具体类型形状，无法解析实例 owner"))?;
    let concrete = type_id(ids, ty)?;
    nominal_by_type
        .get(&concrete)
        .copied()
        .ok_or_else(|| error_without_module("CAN058", format!("聚合具体类型 {concrete:?}（{ty:?}）缺少实例级 nominal 身份")))
}

/// 把声明级字段身份改写成 owner 实例的字段身份；缺登记直接失败关闭。
fn resolve_instance_field(
    owner: NominalInstanceId,
    field: FieldId,
    field_by_owner: &BTreeMap<(NominalInstanceId, FieldId), FieldId>,
    field_names: &BTreeMap<FieldId, String>,
) -> Result<FieldId, StructuredDiagnosticSet> {
    field_by_owner.get(&(owner, field)).copied().ok_or_else(|| {
        let name = field_names.get(&field).map(String::as_str).unwrap_or("<未登记字段>");
        error_without_module("CAN058", format!("聚合实例 {owner:?} 缺少字段 `{name}` 的实例级字段身份"))
    })
}

fn lower_operation(
    operation: &MirOperation,
    results: &[crate::compiler::mir::MirValueRef],
    value_types: &BTreeMap<crate::compiler::mir::MirValueRef, ValkyrieType>,
    ids: &BTreeMap<ValkyrieType, TypeId>,
    nominal_by_type: &BTreeMap<TypeId, NominalInstanceId>,
    fields: &BTreeMap<FieldId, FieldRecord>,
    field_by_owner: &BTreeMap<(NominalInstanceId, FieldId), FieldId>,
    field_names: &BTreeMap<FieldId, String>,
) -> Result<CanonicalOperation, StructuredDiagnosticSet> {
    let value = |operand: &MirOperand| match operand {
        MirOperand::Value(value) => MirValueId::from_index(value.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出")),
        _ => Err(error_without_module("CAN006", "操作数不是已定义 SSA 值")),
    };
    match operation {
        MirOperation::Call { callee: MirOperand::Callable(identity), arguments } => Ok(CanonicalOperation::Invoke {
            callee: CanonicalCallee::Item(*identity),
            arguments: arguments.iter().map(value).collect::<Result<_, _>>()?,
        }),
        MirOperation::Call { callee: MirOperand::Value(callee), arguments } => Ok(CanonicalOperation::Invoke {
            callee: CanonicalCallee::Value(
                MirValueId::from_index(callee.0).ok_or_else(|| error_without_module("CAN008", "函数值 callee identity 溢出"))?,
            ),
            arguments: arguments.iter().map(value).collect::<Result<_, _>>()?,
        }),
        // trait 方法槽 witness 在 canonical 保留稳定分派身份：self 类型已具体化时记下 TypeId，
        // 仍是开放泛型时保留 `None`。不得按名字猜实现，也不得退回 `Unit`。
        MirOperation::Call { callee: MirOperand::Witness(witness), arguments } => Ok(CanonicalOperation::Invoke {
            callee: CanonicalCallee::Witness(CanonicalWitness {
                trait_declaration: witness.trait_declaration,
                method_declaration: witness.method_declaration,
                self_type: ids.get(&witness.self_type).copied(),
            }),
            arguments: arguments.iter().map(value).collect::<Result<_, _>>()?,
        }),
        MirOperation::StoreVar { name, value: stored, ty } => Ok(CanonicalOperation::StoreVar {
            name: name.clone(),
            value: value(stored)?,
            ty: ty.as_ref().map(|ty| type_id(ids, ty)).transpose()?,
        }),
        MirOperation::Copy { source } => Ok(CanonicalOperation::Copy { source: value(source)? }),
        MirOperation::Cast { source, ty } => Ok(CanonicalOperation::Cast { source: value(source)?, ty: type_id(ids, ty)? }),
        MirOperation::AggregateCopy { source, dest } => {
            Ok(CanonicalOperation::AggregateCopy { source: value(source)?, destination: value(dest)? })
        }
        MirOperation::LoadConstant { constant, .. } => Ok(CanonicalOperation::LoadConstant { constant: lower_constant(constant)? }),
        MirOperation::ArrayGet { array, index } => Ok(CanonicalOperation::ArrayGet { array: value(array)?, index: value(index)? }),
        MirOperation::ArraySet { array, index, value: stored } => {
            Ok(CanonicalOperation::ArraySet { array: value(array)?, index: value(index)?, value: value(stored)? })
        }
        MirOperation::ArrayLength { array } => Ok(CanonicalOperation::ArrayLength { array: value(array)? }),
        MirOperation::ArrayPush { array, value: pushed } => Ok(CanonicalOperation::ArrayPush { array: value(array)?, value: value(pushed)? }),
        MirOperation::NullIsNull { value: operand } => Ok(CanonicalOperation::NullIsNull { value: value(operand)? }),
        MirOperation::NullUnwrap { value: operand } => Ok(CanonicalOperation::NullUnwrap { value: value(operand)? }),
        MirOperation::ArrayNew { array_type, length, initialization } => Ok(CanonicalOperation::ArrayNew {
            array_type: type_id(ids, array_type)?,
            length: value(length)?,
            initialization: match initialization {
                crate::compiler::mir::ArrayInitialization::Default => CanonicalArrayInitialization::Default,
                crate::compiler::mir::ArrayInitialization::Fill(fill) => CanonicalArrayInitialization::Fill(value(fill)?),
            },
        }),
        MirOperation::ArrayFromElements { array_type, elements } => Ok(CanonicalOperation::ArrayFromElements {
            array_type: type_id(ids, array_type)?,
            elements: elements.iter().map(value).collect::<Result<_, _>>()?,
        }),
        MirOperation::TupleNew { fields } => Ok(CanonicalOperation::TupleNew {
            element_types: match results.first().and_then(|result| value_types.get(result)) {
                Some(ValkyrieType::Tuple(types)) => types.iter().map(|ty| type_id(ids, ty)).collect::<Result<_, _>>()?,
                _ => {
                    return Err(error_without_module("CAN017", "元组构造结果缺少完整 Tuple 类型"));
                }
            },
            fields: fields.iter().map(value).collect::<Result<_, _>>()?,
        }),
        MirOperation::SumNew { nominal, variant, payload_type, payload, .. } => {
            let result_ty = results.first().and_then(|result| value_types.get(result));
            let concrete = resolve_sum_nominal(*nominal, result_ty, ids, nominal_by_type)?;
            let _ = payload_type;
            Ok(CanonicalOperation::SumNew { nominal: concrete, variant: *variant, payload: payload.as_ref().map(value).transpose()? })
        }
        MirOperation::SumPayloadGet { nominal, variant, object, .. } => {
            let object_ty = match object {
                MirOperand::Value(value) => value_types.get(value),
                _ => None,
            };
            let concrete = resolve_sum_nominal(*nominal, object_ty, ids, nominal_by_type)?;
            Ok(CanonicalOperation::SumPayloadGet { nominal: concrete, variant: *variant, object: value(object)? })
        }
        MirOperation::SumVariantIs { nominal, variant, object, .. } => {
            let object_ty = match object {
                MirOperand::Value(value) => value_types.get(value),
                _ => None,
            };
            let concrete = resolve_sum_nominal(*nominal, object_ty, ids, nominal_by_type)?;
            Ok(CanonicalOperation::SumVariantIs { nominal: concrete, variant: *variant, object: value(object)? })
        }
        MirOperation::StructNew { nominal: _declaration, fields: values } => {
            // MIR 构造指令携带声明 nominal；实例身份只能由结果静态类型决定。
            let result_ty = results.first().and_then(|result| value_types.get(result));
            let owner = resolve_aggregate_nominal(result_ty, ids, nominal_by_type)?;
            let fields = values
                .iter()
                .map(|(field, operand)| {
                    let resolved = resolve_instance_field(owner, *field, field_by_owner, field_names)?;
                    if !fields.contains_key(&resolved) {
                        return Err(error_without_module("CAN024", format!("未解析字段 identity: {resolved:?}")));
                    }
                    Ok((resolved, value(operand)?))
                })
                .collect::<Result<_, StructuredDiagnosticSet>>()?;
            Ok(CanonicalOperation::StructNew { nominal: owner, fields })
        }
        MirOperation::FieldGet { object, field } => {
            let object_ty = match object {
                MirOperand::Value(value) => value_types.get(value),
                _ => None,
            };
            let owner = resolve_aggregate_nominal(object_ty, ids, nominal_by_type)?;
            let resolved = resolve_instance_field(owner, *field, field_by_owner, field_names)?;
            if !fields.contains_key(&resolved) {
                return Err(error_without_module("CAN027", format!("未解析字段 identity: {resolved:?}")));
            }
            Ok(CanonicalOperation::FieldGet { object: value(object)?, field: resolved })
        }
        MirOperation::FieldSet { object, field, value: stored } => {
            let object_ty = match object {
                MirOperand::Value(value) => value_types.get(value),
                _ => None,
            };
            let owner = resolve_aggregate_nominal(object_ty, ids, nominal_by_type)?;
            let resolved = resolve_instance_field(owner, *field, field_by_owner, field_names)?;
            if !fields.contains_key(&resolved) {
                return Err(error_without_module("CAN030", format!("未解析字段 identity: {resolved:?}")));
            }
            Ok(CanonicalOperation::FieldSet { object: value(object)?, field: resolved, value: value(stored)? })
        }
        _ => Err(error_without_module("CAN009", format!("操作没有无损 canonical 形状: {operation:?}"))),
    }
}

fn lower_terminator(terminator: &MirTerminator) -> Result<CanonicalTerminator, StructuredDiagnosticSet> {
    let value = |operand: &MirOperand| match operand {
        MirOperand::Value(value) => MirValueId::from_index(value.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出")),
        _ => Err(error_without_module("CAN010", "终结符操作数不是 SSA 值")),
    };
    match terminator {
        MirTerminator::Return { value: Some(value_operand) } => Ok(CanonicalTerminator::Return { value: Some(value(value_operand)?) }),
        MirTerminator::Return { value: None } => Ok(CanonicalTerminator::Return { value: None }),
        MirTerminator::Jump { target, arguments } => Ok(CanonicalTerminator::Jump {
            target: CanonicalBlockId(target.0),
            arguments: arguments.iter().map(value).collect::<Result<_, _>>()?,
        }),
        MirTerminator::Branch { condition, then_target, else_target } => Ok(CanonicalTerminator::Branch {
            condition: value(condition)?,
            then_target: CanonicalBlockId(then_target.0),
            else_target: CanonicalBlockId(else_target.0),
        }),
        MirTerminator::PerformEffect { effect, payload, resume_target } => Ok(CanonicalTerminator::PerformEffect {
            effect: lower_effect(*effect),
            payload: payload.as_ref().map(value).transpose()?,
            resume_target: CanonicalBlockId(resume_target.0),
        }),
        MirTerminator::StateDispatch { state, cases, default_target } => Ok(CanonicalTerminator::StateDispatch {
            state: MirValueId::from_index(state.0).ok_or_else(|| error_without_module("CAN004", "SSA value identity 溢出"))?,
            cases: cases.iter().map(|(state, target)| (*state, CanonicalBlockId(target.0))).collect(),
            default_target: CanonicalBlockId(default_target.0),
        }),
        MirTerminator::YieldToRuntime { effect, payload, resume_state } => Ok(CanonicalTerminator::YieldToRuntime {
            effect: lower_effect(*effect),
            payload: payload.as_ref().map(value).transpose()?,
            resume_state: *resume_state,
        }),
        MirTerminator::Unreachable => Ok(CanonicalTerminator::Unreachable),
    }
}

fn lower_effect(effect: crate::compiler::mir::MirEffectKind) -> CanonicalEffectKind {
    use crate::compiler::mir::MirEffectKind;
    match effect {
        MirEffectKind::Raise => CanonicalEffectKind::Raise,
        MirEffectKind::Yield => CanonicalEffectKind::Yield,
        MirEffectKind::DelegateYield => CanonicalEffectKind::DelegateYield,
        MirEffectKind::Await => CanonicalEffectKind::Await,
        MirEffectKind::AsyncSpawn => CanonicalEffectKind::AsyncSpawn,
        MirEffectKind::AsyncBlock => CanonicalEffectKind::AsyncBlock,
    }
}

fn lower_constant(constant: &MirConstant) -> Result<CanonicalConstant, StructuredDiagnosticSet> {
    match constant {
        MirConstant::Int(value) => Ok(CanonicalConstant::Int(*value)),
        MirConstant::Bool(value) => Ok(CanonicalConstant::Bool(*value)),
        MirConstant::Utf8(value) => Ok(CanonicalConstant::Utf8(value.clone())),
        MirConstant::Utf16(value) => Ok(CanonicalConstant::Utf16(value.clone())),
        MirConstant::Unit => Ok(CanonicalConstant::Unit),
        MirConstant::Float64(_) => Err(error_without_module("CAN012", "浮点常量没有 canonical 形状")),
    }
}
