use crate::compiler::{
    hir::{is_nullable_type, lowering::integer_literal::settle_integer},
    types::hir::{HirLiteral, HirResolvedCall, HirTextEncoding, ValkyrieType},
};

use crate::compiler::mir::ssa::{MirConstant, MirOperand, infer_builder_operand_type};

use crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type;

pub(crate) fn lower_literal(
    literal: &HirLiteral,
    expected_type: Option<&ValkyrieType>,
    structs: &[crate::compiler::mir::ssa::MirStruct],
) -> Result<(MirConstant, Option<ValkyrieType>), String> {
    Ok(match literal {
        HirLiteral::Integer(value) => {
            // 宽度、有符号性与范围由 HIR 的 `integer_literal` 唯一结算；
            // MIR 只消费已冻结的「位模式 + 类型」，不在此另写默认宽度规则。
            let (bits, ty) = settle_integer(*value, expected_type)?;
            (MirConstant::Int(bits), Some(ty))
        }
        HirLiteral::Float64(value) => (MirConstant::Float64(*value), Some(ValkyrieType::Float64)),
        // 字符字面量：定型已由 HIR settle 冻结为 `char`（u32 码位，§4.12 字符合同），
        // MIR 只编码既定值——整数常量 + `Character` 类型，不再按期望形参重选 char/text。
        HirLiteral::Character(value) => (MirConstant::Int(i64::from(*value)), Some(ValkyrieType::Character)),
        // 字节字面量：形参为整数类型时写形参类型，否则默认 `u8`。
        HirLiteral::Byte(value) => (
            MirConstant::Int(*value as i64),
            Some(match expected_type {
                Some(
                    ty @ (ValkyrieType::Integer8 { .. }
                    | ValkyrieType::Integer16 { .. }
                    | ValkyrieType::Integer32 { .. }
                    | ValkyrieType::Integer64 { .. }
                    | ValkyrieType::Integer128 { .. }),
                ) => ty.clone(),
                Some(ty @ (ValkyrieType::Named(_) | ValkyrieType::Nominal { .. })) if is_integer_literal_formal_type(ty, &[]) => ty.clone(),
                _ => ValkyrieType::Integer8 { signed: false },
            }),
        ),
        HirLiteral::Bool(value) => (MirConstant::Bool(*value), Some(ValkyrieType::Boolean)),
        HirLiteral::String(value) => {
            // 插值必须在 HIR 脱糖为已绑定的 display 调用树；MIR 不渲染 `${...}` 或任何占位文本。
            if !value.is_plain_text() {
                return Err("internal invariant: string interpolation reached MIR without HIR desugaring".to_string());
            }
            let Some(encoding) = value.text_encoding
            else {
                return Err("internal invariant: string literal reached MIR without a settled text encoding".to_string());
            };
            let text = value
                .segments
                .iter()
                .map(|segment| match segment {
                    crate::compiler::types::hir::HirStringSegment::Text(text) => text.as_str(),
                    crate::compiler::types::hir::HirStringSegment::Interpolation { .. } => "",
                })
                .collect::<String>();
            match encoding {
                HirTextEncoding::Utf8 => (MirConstant::Utf8(text), Some(ValkyrieType::Utf8)),
                HirTextEncoding::Utf16 => (MirConstant::Utf16(text), Some(ValkyrieType::Utf16)),
            }
        }
        HirLiteral::Null => (
            MirConstant::Null,
            // 空值的 SSA 类型只能来自上下文 `T?`；没有 nullable 上下文时保持未定型，
            // 由 SSA 校验结构化拒绝，禁止退回 Unit 或猜一个名义 Option。
            expected_type.filter(|ty| is_nullable_type(ty)).cloned(),
        ),
        HirLiteral::Unit => (MirConstant::Unit, Some(ValkyrieType::Unit)),
    })
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
) -> Option<(String, Vec<ValkyrieType>, String, Option<ValkyrieType>, Vec<MirOperand>, bool)> {
    let owner = call.sum_owner?;
    let variant_declaration = call.sum_variant?;
    let sum = sum_types.iter().find(|item| item.declaration == Some(owner))?;
    let variant = sum.variants.iter().find(|item| item.declaration == Some(variant_declaration))?;
    let (sum_type, type_args) = match &call.return_type {
        ValkyrieType::Nominal { declaration, arguments, .. } if Some(*declaration) == sum.declaration => (sum.name.clone(), arguments.clone()),
        _ => return None,
    };
    if call.return_type.nominal_declaration() != sum.declaration {
        return None;
    }
    // 多字段变体的 payload 是名义变体记录：全部实参按声明顺序构成记录字段，
    // payload 类型只能由声明 + 枚举实参代入，不能取首个字段类型伪装。
    let multi_field = variant.fields.len() > 1;
    let payload_type = if multi_field { sum.instantiate_payload(variant, &type_args).flatten() } else { call.parameter_types.first().cloned() };
    Some((sum_type, type_args, variant.name.clone(), payload_type, arguments.to_vec(), multi_field))
}

pub(crate) fn type_args_from_sum_shaped(ty: &ValkyrieType) -> Vec<ValkyrieType> {
    match ty {
        ValkyrieType::Nominal { arguments, .. } => arguments.clone(),
        ValkyrieType::Apply(_, args) => args.clone(),
        ValkyrieType::Nullable(inner) => vec![(**inner).clone()],
        _ => Vec::new(),
    }
}
