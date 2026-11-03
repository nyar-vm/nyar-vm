//! 操作数静态类型推断与调用类型兼容判定。

use crate::compiler::{
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
pub(super) fn mir_return_operand_type_compatible(actual: &ValkyrieType, expected: &ValkyrieType) -> bool {
    actual == expected
        || primitive_named_equivalent(actual, expected)
        || primitive_named_equivalent(expected, actual)
        || bare_named_matches_generic_apply_return(expected, actual)
        // `T` 可隐式提升为 `T?`：非空值就是同一引用的 Some 侧，不构造名义 Option 包装。
        || matches!(expected, ValkyrieType::Nullable(inner) if mir_return_operand_type_compatible(actual, inner))
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
/// 与非文本名义依旧失败闭合。
pub(super) fn text_encoding_matches_nominal(declared: &ValkyrieType, actual: &ValkyrieType, structs: &[crate::compiler::mir::MirStruct]) -> bool {
    let ValkyrieType::Nominal { declaration, arguments, .. } = declared
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
    match actual {
        ValkyrieType::Utf8 => matches!(name, "text" | "utf8" | "Utf8Text" | "std::text::text" | "std.text.text"),
        ValkyrieType::Utf16 => matches!(name, "text" | "utf16" | "Utf16Text" | "std::text::text" | "std.text.text"),
        _ => false,
    }
}

/// 检查两个类型是否是同一个 primitive numeric 的不同表示形式：
/// `Named("i32")` ↔ `Integer32 { .. }`、`Named("f64")` ↔ `Float64` 等。
fn primitive_named_equivalent(a: &ValkyrieType, b: &ValkyrieType) -> bool {
    fn offset_of(ty: &ValkyrieType) -> Option<u32> {
        Some(match ty {
            ValkyrieType::Boolean => 0,
            ValkyrieType::Integer8 { .. } => 1,
            ValkyrieType::Integer16 { .. } => 2,
            ValkyrieType::Integer32 { .. } => 3,
            ValkyrieType::Integer64 { .. } => 4,
            ValkyrieType::Integer128 { .. } => 5,
            ValkyrieType::Float32 => 6,
            ValkyrieType::Float64 => 7,
            ValkyrieType::Named(name) => match name.as_str() {
                "boolean" | "Bool" | "bool" => 0,
                "i8" => 1,
                "i16" => 2,
                "i32" | "int" | "Int" => 3,
                "i64" | "long" | "Long" => 4,
                "i128" => 5,
                "f32" | "float" | "Float" => 6,
                "f64" | "double" | "Double" => 7,
                _ => return None,
            },
            _ => return None,
        })
    }
    offset_of(a).is_some_and(|off_a| offset_of(b).is_some_and(|off_b| off_a == off_b))
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
