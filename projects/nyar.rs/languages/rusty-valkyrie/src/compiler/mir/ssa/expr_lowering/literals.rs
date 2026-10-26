use crate::compiler::{
    hir::is_nullable_type,
    types::{
        Identifier, NamePath,
        hir::{FunctionType, HirCallableDomain, HirExpr, HirExprKind, HirLiteral, HirResolvedCall, ValkyrieType},
    },
};
use nyar_types::IntrinsicId;

use crate::compiler::mir::ssa::{MirConstant, MirOperand, expr_helpers::named_type_name, infer_builder_operand_type};

pub(crate) fn lower_literal(literal: &HirLiteral, expected_type: Option<&ValkyrieType>) -> (MirConstant, Option<ValkyrieType>) {
    match literal {
        HirLiteral::Integer64(value) => (
            MirConstant::Int(*value),
            Some(match expected_type {
                Some(
                    ty @ (ValkyrieType::Integer8 { .. }
                    | ValkyrieType::Integer16 { .. }
                    | ValkyrieType::Integer32 { .. }
                    | ValkyrieType::Integer64 { .. }
                    | ValkyrieType::Integer128 { .. }),
                ) => ty.clone(),
                // Named 数值形参（如 `usize`）在无 MirStruct 表时也可直接特化；Nominal 由调用方用声明表覆盖。
                Some(ty @ ValkyrieType::Named(_)) if crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(ty, &[]) => {
                    ty.clone()
                }
                _ if *value >= i32::MIN as i64 && *value <= i32::MAX as i64 => ValkyrieType::Integer32 { signed: true },
                _ => ValkyrieType::Integer64 { signed: true },
            }),
        ),
        HirLiteral::Float64(value) => (MirConstant::Float64(*value), Some(ValkyrieType::Float64)),
        HirLiteral::Bool(value) => (MirConstant::Bool(*value), Some(ValkyrieType::Boolean)),
        HirLiteral::String(value) => (
            MirConstant::Utf8(
                value
                    .segments
                    .iter()
                    .map(|segment| match segment {
                        crate::compiler::types::hir::HirStringSegment::Text(text) => text.clone(),
                        crate::compiler::types::hir::HirStringSegment::Interpolation { expr, .. } => {
                            format!("${{{}}}", render_interpolation_expr(expr))
                        }
                    })
                    .collect::<String>(),
            ),
            Some(ValkyrieType::Utf8),
        ),
        HirLiteral::Unit => (MirConstant::Unit, Some(ValkyrieType::Unit)),
    }
}

fn render_interpolation_expr(expr: &HirExpr) -> String {
    match &expr.kind {
        HirExprKind::Variable(identifier) => identifier.name.to_string(),
        HirExprKind::Path(path) => path.to_string(),
        HirExprKind::Literal(HirLiteral::Integer64(value)) => value.to_string(),
        HirExprKind::Literal(HirLiteral::Bool(value)) => value.to_string(),
        HirExprKind::Literal(HirLiteral::String(value)) => value
            .segments
            .iter()
            .map(|segment| match segment {
                crate::compiler::types::hir::HirStringSegment::Text(text) => text.clone(),
                crate::compiler::types::hir::HirStringSegment::Interpolation { .. } => "${...}".to_string(),
            })
            .collect(),
        HirExprKind::Call { callee, .. } => format!("{}(...)", render_interpolation_expr(callee)),
        _ => "...".to_string(),
    }
}

/// Prefer the contextual sum apply when both sides already share Nominal declaration identity.
/// payload 须由调用方经 `parameter_types` / `instantiate_payload` 提供，禁止变体短名认亲。
pub(crate) fn concretize_variant_constructor_types(
    ctor_return: &ValkyrieType,
    payload_type: Option<ValkyrieType>,
    expected_type: Option<&ValkyrieType>,
    arguments: &[MirOperand],
    value_types: &std::collections::BTreeMap<crate::compiler::mir::ssa::MirValueRef, ValkyrieType>,
) -> (ValkyrieType, Option<ValkyrieType>) {
    let return_type = match expected_type {
        Some(expected) if same_sum_owner(ctor_return, expected) => expected.clone(),
        _ => ctor_return.clone(),
    };
    let from_argument = arguments.first().and_then(|argument| infer_builder_operand_type(argument, value_types));
    let payload_type = payload_type.or(from_argument);
    (return_type, payload_type)
}

fn same_sum_owner(left: &ValkyrieType, right: &ValkyrieType) -> bool {
    match (left.nominal_declaration(), right.nominal_declaration()) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// 失败诊断只引用已冻结的声明身份；缺失时给出稳定占位，禁止从拼写重发现 variant。
pub(crate) fn unresolved_variant_labels(resolved: Option<&HirResolvedCall>) -> (String, String) {
    let sum_type = resolved.and_then(|call| call.sum_owner).map_or_else(|| "<unresolved sum owner>".to_string(), |owner| owner.to_string());
    let variant =
        resolved.and_then(|call| call.sum_variant).map_or_else(|| "<unresolved sum variant>".to_string(), |variant| variant.to_string());
    (sum_type, variant)
}

/// 按 sum_owner + sum_variant + 已冻结实参代入 payload；禁止 Fine/Some/Fail 拼写选槽。
pub(crate) fn instantiated_payload_for_call(
    sum_types: &[crate::compiler::mir::ssa::MirSumDeclaration],
    call: &HirResolvedCall,
    sum_type: &ValkyrieType,
) -> Option<ValkyrieType> {
    let owner = call.sum_owner?;
    let variant_declaration = call.sum_variant?;
    let sum = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(sum_types, owner)?;
    let variant = sum.variants.iter().find(|item| item.declaration == Some(variant_declaration))?;
    let args = type_args_from_sum_shaped(sum_type);
    if args.is_empty() {
        return call.parameter_types.first().cloned().or_else(|| variant.payload_type());
    }
    match sum.instantiate_payload(variant, &args) {
        Some(payload) => payload.or_else(|| call.parameter_types.first().cloned()),
        None => call.parameter_types.first().cloned(),
    }
}

/// 将构造器 payload 写回其声明 binder 对应的 type arg 槽位。
pub(crate) fn apply_payload_to_sum_type_args(
    sum: &crate::compiler::mir::ssa::MirSumDeclaration,
    variant: &crate::compiler::mir::MirSumVariant,
    type_args: &mut Vec<ValkyrieType>,
    payload: &ValkyrieType,
) {
    let Some(declared_payload) = variant.payload_type()
    else {
        return;
    };
    let ValkyrieType::Generic(generic) = declared_payload
    else {
        return;
    };
    let Some(binder) = generic.binder
    else {
        return;
    };
    let Some(index) = sum.generics.iter().position(|item| item.binder == Some(binder))
    else {
        return;
    };
    if index < type_args.len() {
        type_args[index] = payload.clone();
    }
}

/// Map a resolved unite/enum constructor call onto `SumNew` parts.
pub(crate) fn sum_new_parts_from_constructor(
    call: &HirResolvedCall,
    arguments: &[MirOperand],
    sum_types: &[crate::compiler::mir::ssa::MirSumDeclaration],
) -> Option<(String, Vec<ValkyrieType>, String, Option<ValkyrieType>, Option<MirOperand>)> {
    let owner = call.sum_owner?;
    let variant_declaration = call.sum_variant?;
    let sum = sum_types.iter().find(|item| item.declaration == Some(owner))?;
    let variant = sum.variants.iter().find(|item| item.declaration == Some(variant_declaration))?;
    let (sum_type, type_args) = match &call.return_type {
        ValkyrieType::Nominal { declaration, arguments } if Some(*declaration) == sum.declaration => (sum.name.clone(), arguments.clone()),
        _ => return None,
    };
    if call.return_type.nominal_declaration() != sum.declaration {
        return None;
    }
    let payload_type = call.parameter_types.first().cloned();
    let payload = match arguments.len() {
        0 => None,
        1 => Some(arguments[0].clone()),
        _ => arguments.first().cloned(),
    };
    Some((sum_type, type_args, variant.name.clone(), payload_type, payload))
}

pub(crate) fn type_args_from_sum_shaped(ty: &ValkyrieType) -> Vec<ValkyrieType> {
    match ty {
        ValkyrieType::Nominal { arguments, .. } => arguments.clone(),
        ValkyrieType::Apply(_, args) => args.clone(),
        ValkyrieType::Nullable(inner) => vec![(**inner).clone()],
        _ => Vec::new(),
    }
}

/// ?????? overload ????`Apply(Option, [AutoType])`?? payload ??????
pub(crate) fn refine_auto_sum_instantiation_args(type_args: &[ValkyrieType], payload_type: Option<&ValkyrieType>) -> Vec<ValkyrieType> {
    type_args
        .iter()
        .map(|arg| if matches!(arg, ValkyrieType::AutoType) { payload_type.cloned().unwrap_or_else(|| arg.clone()) } else { arg.clone() })
        .collect()
}

pub(crate) fn refine_auto_apply_type(ty: &ValkyrieType, payload_type: Option<&ValkyrieType>) -> ValkyrieType {
    match ty {
        ValkyrieType::Nominal { declaration, arguments } if arguments.iter().any(|arg| matches!(arg, ValkyrieType::AutoType)) => {
            ValkyrieType::Nominal { declaration: *declaration, arguments: refine_auto_sum_instantiation_args(arguments, payload_type) }
        }
        ValkyrieType::Apply(base, args) if args.iter().any(|arg| matches!(arg, ValkyrieType::AutoType)) => {
            ValkyrieType::Apply(base.clone(), refine_auto_sum_instantiation_args(args, payload_type))
        }
        _ => ty.clone(),
    }
}
