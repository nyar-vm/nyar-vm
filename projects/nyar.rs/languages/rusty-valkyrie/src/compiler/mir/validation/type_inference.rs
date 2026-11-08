//! 操作数静态类型推断与调用类型兼容判定。

use crate::compiler::{
    hir::type_lowering::primitive_surface_type,
    mir::{MirConstant, MirEffectKind, MirFunction, MirOperand, MirValueOrigin},
    types::hir::ValkyrieType,
};

pub(super) fn infer_operand_static_type(function: &MirFunction, operand: &MirOperand) -> Option<ValkyrieType> {
    match operand {
        MirOperand::Constant(constant) => Some(infer_constant_type(constant)),
        MirOperand::Value(value_ref) => function.value_types.get(value_ref).cloned().or_else(|| {
            let value = function.values.iter().find(|value| value.id == *value_ref)?;
            match &value.origin {
                MirValueOrigin::Parameter { index, .. } => function.param_types.get(*index).cloned(),
                _ => None,
            }
        }),
        MirOperand::Callable(_) | MirOperand::Witness(_) => None,
    }
}

pub(super) fn infer_effect_resume_static_type(effect: MirEffectKind, payload_type: Option<&ValkyrieType>) -> Option<ValkyrieType> {
    match effect {
        MirEffectKind::Yield | MirEffectKind::DelegateYield => Some(ValkyrieType::Unit),
        MirEffectKind::Await | MirEffectKind::AsyncBlock => payload_type.and_then(future_resume_type),
        MirEffectKind::AsyncSpawn => None,
        // `raise` 的恢复类型来自 `Effectful::Resume`；当前 HIR 没有该关联类型的
        // 实现事实，校验阶段不猜测恢复参数类型。
        MirEffectKind::Raise => None,
    }
}

pub(super) fn infer_constant_type(constant: &MirConstant) -> ValkyrieType {
    match constant {
        MirConstant::Int(_) => ValkyrieType::Integer64 { signed: true },
        MirConstant::Float64(_) => ValkyrieType::Float64,
        MirConstant::Bool(_) => ValkyrieType::Boolean,
        MirConstant::Utf8(_) => ValkyrieType::Utf8,
        MirConstant::Utf16(_) => ValkyrieType::Utf16,
        MirConstant::Unit => ValkyrieType::Unit,
        // 空值本身没有独立名义类型；其类型由 `LoadConstant.ty` 的 `T?` 上下文给出。
        // 直接作为常量操作数出现时只保留空值身份，不猜 `Option` 载体。
        MirConstant::Null => ValkyrieType::Named(crate::compiler::types::Identifier::new("null")),
    }
}

pub(super) fn future_resume_type(ty: &ValkyrieType) -> Option<ValkyrieType> {
    match ty {
        ValkyrieType::Apply(base, arguments) if arguments.len() == 1 && matches!(named_type_name(base), Some("Future" | "Promise")) => {
            arguments.first().cloned()
        }
        _ => None,
    }
}

pub(super) fn generator_source_type(ty: &ValkyrieType) -> Option<ValkyrieType> {
    match ty {
        ValkyrieType::Apply(base, arguments)
            if !arguments.is_empty() && matches!(named_type_name(base), Some("Generator" | "Iterator" | "Coroutine")) =>
        {
            arguments.first().cloned()
        }
        ValkyrieType::Named(name) if matches!(name.as_str(), "Generator" | "Iterator" | "Coroutine") => Some(ValkyrieType::Unit),
        _ => None,
    }
}

/// MIR 与源码均使用 `Void` 表达底类型，不保留名称别名。
///
/// 同时允许 primitive numeric 的两种表示形式等价：
/// `Named("i32")`（前端声明）↔ `Integer32 { signed: true }`（底层 canonical 变体）。
pub(super) fn mir_return_operand_type_compatible(
    actual: &ValkyrieType,
    expected: &ValkyrieType,
    structs: &[crate::compiler::mir::MirStruct],
) -> bool {
    actual == expected
        || primitive_named_equivalent(actual, expected)
        || primitive_named_equivalent(expected, actual)
        || bare_named_matches_generic_apply_return(expected, actual)
        // 文本常量只携带编码（`Utf8`/`Utf16`），声明侧 `utf8` 是锚定该编码的名义别名：
        // 返回值同口径归一，跨族编码与非文本名义仍失败闭合。
        || text_carriers_equivalent(expected, actual, structs)
        // `T` 可隐式提升为 `T?`：非空值就是同一引用的 Some 侧，不构造名义 Option 包装。
        || matches!(expected, ValkyrieType::Nullable(inner) if mir_return_operand_type_compatible(actual, inner, structs))
}

/// Seed callable signatures stay generic in `MirFunction`, while HIR call sites carry instantiated types.
pub(super) fn call_site_type_compatible(declared: &ValkyrieType, actual: &ValkyrieType) -> bool {
    declared == actual
        || primitive_named_equivalent(declared, actual)
        || primitive_named_equivalent(actual, declared)
        || crate::compiler::mir::ssa::generic_call_instantiation::type_instantiates_generic_signature(declared, actual)
}

/// 文本编码与名义声明的调用边界兼容：字符串常量按 SMIR007 文本常量条款只携带
/// 编码类型（`Utf8`/`Utf16`），源级 `type utf8 = Utf8Text` 别名把该编码锚定到名义
/// 声明上，因此编码实参与同族文本名义形参兼容；跨族（`Utf16` 实参对 `utf8` 形参）
/// 与非文本名义依旧失败闭合。判定方向无关：HIR 的 `text_carriers_equivalent` 同一
/// 名集，生成单元里形参是内建 carrier、实参来自名义字段时同样成立。
pub(super) fn text_carriers_equivalent(declared: &ValkyrieType, actual: &ValkyrieType, structs: &[crate::compiler::mir::MirStruct]) -> bool {
    text_builtin_anchors_nominal_declaration(declared, actual, structs)
        || text_builtin_anchors_nominal_declaration(actual, declared, structs)
}

/// 一侧是内建编码 carrier、另一侧是锚定该编码的名义声明时成立。
fn text_builtin_anchors_nominal_declaration(
    builtin: &ValkyrieType,
    declaration: &ValkyrieType,
    structs: &[crate::compiler::mir::MirStruct],
) -> bool {
    let ValkyrieType::Nominal { declaration, arguments, .. } = declaration
    else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    let Some(item) = crate::compiler::mir::MirStruct::find_by_declaration(structs, *declaration)
    else {
        return false;
    };
    let name = item.name.as_str();
    crate::compiler::text_carrier::carrier_matches_declaration_name(builtin, name)
}

/// 检查两个类型是否是同一个 primitive 的不同表示形式：
/// `Named("i32")` ↔ `Integer32 { .. }`、`Named("f64")` ↔ `Float64` 等。
///
/// 归一表唯一产生点在 `hir::type_lowering::primitive_surface_type`；本函数只比较归一秒。
fn primitive_named_equivalent(a: &ValkyrieType, b: &ValkyrieType) -> bool {
    match (primitive_surface_type(a), primitive_surface_type(b)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub(super) fn bare_named_matches_generic_apply_return(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    match (expected, actual) {
        (ValkyrieType::Apply(base, _), ValkyrieType::Named(name)) | (ValkyrieType::Named(name), ValkyrieType::Apply(base, _)) => {
            named_type_name(base).is_some_and(|base_name| base_name == name.as_str())
        }
        _ => false,
    }
}

pub(super) fn named_type_name(ty: &ValkyrieType) -> Option<&str> {
    match ty {
        ValkyrieType::Named(name) => Some(name.as_str()),
        ValkyrieType::Apply(base, _) => named_type_name(base),
        _ => None,
    }
}
