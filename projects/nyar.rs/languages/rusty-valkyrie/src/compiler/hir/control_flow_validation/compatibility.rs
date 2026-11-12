use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::{
        is_nullable_type, is_option_apply_type, is_result_apply_type,
        lowering::{
            HirError,
            errors::{ControlFlowOp, LoopJumpOperation, PropagateProtocol, RequiredContext},
        },
        nullable_payload_type,
        type_lowering::primitive_surface_type,
    },
    semantic::control_flow::{ControlFlowContext, TryScopeData},
    types::{
        Identifier, NamePath,
        hir::{
            HirBlock, HirExpr, HirExprKind, HirFunction, HirLiteral, HirMatchArm, HirModule, HirPattern, HirStatement, HirStatementKind,
            ValkyrieType,
        },
    },
};

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use super::{control_flow_source, expr_diagnostic_span};

use crate::compiler::hir::control_flow_validation::state::HirValidationState;

/// 判断控制流校验中期望类型与实参类型是否兼容（不按名字回退 builtin / 泛型 / Self）。
pub fn control_flow_value_type_compatible(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    control_flow_value_type_compatible_with_primitives(expected, actual, None)
}

pub fn control_flow_value_type_compatible_with_primitives(
    expected: &ValkyrieType,
    actual: &ValkyrieType,
    state: Option<&HirValidationState>,
) -> bool {
    expected == actual
        || matches!(expected, ValkyrieType::AutoType)
        || matches!(actual, ValkyrieType::AutoType)
        || integer_carriers_share_identity(expected, actual)
        || primitive_named_equivalent(expected, actual)
        || array_types_compatible(expected, actual)
        || nullable_return_compatible(expected, actual)
        || nominal_apply_types_compatible(expected, actual)
        || state.is_some_and(|state| primitive_boolean_compatible(expected, actual, state))
}

/// 检查两个类型是否是同一个 primitive 的不同表示形式：
/// `Named("i32")` ↔ `Integer32 { .. }`、`Named("f64")` ↔ `Float64` 等。
///
/// 身份键是 `hir::type_lowering::primitive_surface_type` 的归一秒（唯一产生点），
/// 自带符号性：`i32` 与 `u32` 不得因表示形式合并。
/// `bool` 不在本判定内：布尔的名义等价只承认 `boolean_declarations` 证据
/// （见 `primitive_boolean_compatible`），未绑定声明的 `Named("bool")` 不得顶替 builtin Boolean。
fn primitive_named_equivalent(a: &ValkyrieType, b: &ValkyrieType) -> bool {
    let numeric = |ty: &ValkyrieType| {
        primitive_surface_type(ty).filter(|canonical| !matches!(canonical, ValkyrieType::Boolean | ValkyrieType::Character))
    };
    match (numeric(a), numeric(b)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub fn primitive_boolean_compatible(expected: &ValkyrieType, actual: &ValkyrieType, state: &HirValidationState) -> bool {
    let is_bool_nominal =
        |ty: &ValkyrieType| matches!(ty, ValkyrieType::Nominal { declaration, .. } if state.boolean_declarations.contains(declaration));
    (is_bool_nominal(expected) && matches!(actual, ValkyrieType::Boolean))
        || (is_bool_nominal(actual) && matches!(expected, ValkyrieType::Boolean))
}

pub fn nominal_apply_types_compatible(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    match (expected, actual) {
        (
            ValkyrieType::Nominal { declaration: expected_decl, arguments: expected_args, .. },
            ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args, .. },
        ) if expected_decl == actual_decl && expected_args.len() == actual_args.len() => {
            expected_args.iter().zip(actual_args).all(|(expected, actual)| type_parameters_compatible(expected, actual))
        }
        _ => false,
    }
}

pub fn type_parameters_compatible(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    match (expected, actual) {
        (ValkyrieType::Generic(expected), ValkyrieType::Generic(actual)) if expected.name == actual.name => true,
        _ => expected == actual,
    }
}

pub fn nullable_return_compatible(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    match (nullable_payload_type(expected), nullable_payload_type(actual)) {
        (Some(lhs), Some(rhs)) => lhs == rhs,
        (Some(lhs), None) => lhs == *actual,
        (None, Some(rhs)) => *expected == rhs,
        _ => false,
    }
}

pub fn array_types_compatible(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    match (expected, actual) {
        (ValkyrieType::Array(expected), ValkyrieType::Array(actual)) => {
            expected == actual || matches!(expected.as_ref(), ValkyrieType::AutoType) || matches!(actual.as_ref(), ValkyrieType::AutoType)
        }
        _ => false,
    }
}
/// 标量字面量按期望类型绑定：整数字面量（默认宽度 `i64`）只认数值期望；字节字面量
/// 同源（值域 `u8` 已在 lowering 校验）；字符字面量接受 `char`、数值与文本期望；
/// 恰好一个码位的文本捕获接受 `char` 期望（§4.10 B 上下文定型，引号不预定类型）。
/// 判定与 `overload.rs::specialize_call_literal_exprs` 完全同源：同一份数值名集合
/// （`Named("i32")` 与 `Nominal{ItemId}` 都算数值），同一条件（仅标量字面量）。
pub fn scalar_literal_binds_to(expected: &ValkyrieType, value: Option<&HirExpr>, state: &HirValidationState) -> bool {
    let numeric_expected = match expected {
        ValkyrieType::Nominal { declaration, .. } => state.numeric_declarations.contains(declaration),
        other => crate::compiler::hir::type_relation::is_numeric_type_name_of(other),
    };
    match value.map(|value| &value.kind) {
        Some(HirExprKind::Literal(HirLiteral::Integer(_) | HirLiteral::Byte(_))) => numeric_expected,
        Some(HirExprKind::Literal(HirLiteral::Character(_))) => {
            // 字符字面量在 char/数值期望按码位绑定；文本期望按单字符文本绑定
            // （§4.10 B：`let t: utf8 = 'x'` 成立，引号不预定类型）。
            numeric_expected || crate::compiler::hir::type_relation::is_character_type_name_of(expected) || is_text_expected(expected, state)
        }
        // 未定型文本捕获按唯一绑定规则判定（§4.10 B）：char 期望接受恰好一个码位
        // （两种引号均可），文本期望接受任意捕获，数值期望只接受单引号单码位。
        // 规则本体与 overload/结算共用 `quote_literal::capture_binds`，不另立分支。
        Some(HirExprKind::Literal(HirLiteral::String(literal))) => {
            let kind = if crate::compiler::hir::type_relation::is_character_type_name_of(expected) {
                crate::compiler::hir::lowering::quote_literal::CaptureTargetKind::Char
            }
            else if is_text_expected(expected, state) {
                crate::compiler::hir::lowering::quote_literal::CaptureTargetKind::Text(text_encoding_of_expected(expected, state))
            }
            else if numeric_expected {
                crate::compiler::hir::lowering::quote_literal::CaptureTargetKind::Numeric
            }
            else {
                crate::compiler::hir::lowering::quote_literal::CaptureTargetKind::Other
            };
            crate::compiler::hir::lowering::quote_literal::capture_binds(literal, kind).is_some()
        }
        _ => false,
    }
}

/// 校验侧文本期望的编码载体：`Utf16` 内建落 `Utf16`，其余按声明身份登记的编码；
/// 与 `TypeRelationContext::text_encoding_of` 同源，不读拼写。
fn text_encoding_of_expected(ty: &ValkyrieType, state: &HirValidationState) -> crate::compiler::types::hir::HirTextEncoding {
    match ty {
        ValkyrieType::Utf16 => crate::compiler::types::hir::HirTextEncoding::Utf16,
        ValkyrieType::Nominal { declaration, .. } => {
            state.text_declarations.get(declaration).copied().unwrap_or(crate::compiler::types::hir::HirTextEncoding::Utf8)
        }
        _ => crate::compiler::types::hir::HirTextEncoding::Utf8,
    }
}

/// 校验侧文本期望判定：内建编码 carrier，或本模块声明身份在 `text_declarations` 中的
/// `Nominal`（身份事实与 `text_carrier` 同源）。未解析的 `Named` 拼写不放行。
fn is_text_expected(ty: &ValkyrieType, state: &HirValidationState) -> bool {
    match ty {
        ValkyrieType::Utf8 | ValkyrieType::Utf16 => true,
        ValkyrieType::Nominal { declaration, .. } => state.text_declarations.contains_key(declaration),
        _ => false,
    }
}

pub fn is_integer_type(ty: &ValkyrieType) -> bool {
    matches!(
        ty,
        ValkyrieType::Integer8 { .. }
            | ValkyrieType::Integer16 { .. }
            | ValkyrieType::Integer32 { .. }
            | ValkyrieType::Integer64 { .. }
            | ValkyrieType::Integer128 { .. }
    )
}

/// 整数 carrier 的互容只承认同一名义身份（ADR 0009/0013）：位宽与有符号性都必须相同。
///
/// 不同档位（如 `i32` 与 `i64`、`i32` 与 `u32`）不得因「都是整数」互相顶替。
/// 标量字面量的宽度由期望类型决定，走 `scalar_literal_binds_to`，不在这里放宽。
fn integer_carriers_share_identity(expected: &ValkyrieType, actual: &ValkyrieType) -> bool {
    match (integer_carrier_identity(expected), integer_carrier_identity(actual)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// 整数 carrier 名义身份的构成因子：位宽 + 有符号性。
///
/// 内建标量的 `[primitive]` 声明已在绑定期折叠成 `Integer*` 载体，含义与身份只剩
/// 一个来源；此处仅保留 fixture 专用的 `byte` 宿主绑定身份，真实 std 不声明它。
fn integer_carrier_identity(ty: &ValkyrieType) -> Option<(u16, bool)> {
    Some(match ty {
        ValkyrieType::Integer8 { signed } => (8, *signed),
        ValkyrieType::Integer16 { signed } => (16, *signed),
        ValkyrieType::Integer32 { signed } => (32, *signed),
        ValkyrieType::Integer64 { signed } => (64, *signed),
        ValkyrieType::Integer128 { signed } => (128, *signed),
        ValkyrieType::Nominal { primitive_identity: Some(identity), arguments, .. } if arguments.is_empty() => {
            match identity.rsplit("::").next().unwrap_or(identity) {
                "byte" => (8, false),
                _ => return None,
            }
        }
        _ => return None,
    })
}

pub fn return_without_value_compatible(expected: &ValkyrieType) -> bool {
    matches!(expected, ValkyrieType::Unit | ValkyrieType::Void | ValkyrieType::AutoType)
}

pub fn future_resume_type(ty: &ValkyrieType) -> Option<ValkyrieType> {
    match ty {
        ValkyrieType::Apply(base, arguments) if arguments.len() == 1 && matches!(named_type_name(base), Some("Future" | "Promise")) => {
            arguments.first().cloned()
        }
        _ => None,
    }
}
///
/// witness table 机制不区分 dyn trait 与 impl trait，因此函数返回值可直接写
/// `Iterator<T>` 或 `Generator<T>`，本函数统一提取其类型参数作为 yield 值的期望类型。
pub fn generator_yield_type(ty: &ValkyrieType) -> Option<ValkyrieType> {
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

pub fn named_type_name(ty: &ValkyrieType) -> Option<&str> {
    match ty {
        ValkyrieType::Named(name) => Some(name.as_str()),
        ValkyrieType::Apply(base, _) => named_type_name(base),
        _ => None,
    }
}

/// 类型的用户可读渲染（`i64`、`[T]`、`micro(...) -> ...` 等），供诊断文案使用。
pub fn display_type(ty: &ValkyrieType) -> String {
    match ty {
        ValkyrieType::Void => "void".to_string(),
        ValkyrieType::Unit => "unit".to_string(),
        ValkyrieType::Boolean => "bool".to_string(),
        ValkyrieType::Integer8 { signed } => integer_type_name(*signed, 8),
        ValkyrieType::Integer16 { signed } => integer_type_name(*signed, 16),
        ValkyrieType::Integer32 { signed } => integer_type_name(*signed, 32),
        ValkyrieType::Integer64 { signed } => integer_type_name(*signed, 64),
        ValkyrieType::Integer128 { signed } => integer_type_name(*signed, 128),
        ValkyrieType::Float32 => "f32".to_string(),
        ValkyrieType::Float64 => "f64".to_string(),
        ValkyrieType::Character => "char".to_string(),
        ValkyrieType::Utf8 => "utf8".to_string(),
        ValkyrieType::Utf16 => "utf16".to_string(),
        ValkyrieType::Named(name) => name.to_string(),
        ValkyrieType::QualifiedPath { namespace, name } => ValkyrieType::qualified_path_label(namespace, name),
        ValkyrieType::Nominal { declaration, arguments, .. } => {
            if arguments.is_empty() {
                format!("Nominal({declaration:?})")
            }
            else {
                format!("Nominal({declaration:?})<{}>", arguments.iter().map(display_type).collect::<Vec<_>>().join(", "))
            }
        }
        ValkyrieType::Apply(base, arguments) => {
            format!("{}<{}>", display_type(base), arguments.iter().map(display_type).collect::<Vec<_>>().join(", "))
        }
        ValkyrieType::Generic(generic) => format!("{}[{:?}]", generic.name, generic.binder),
        ValkyrieType::Function(function) => format!(
            "micro({}) -> {}",
            function.params.iter().map(display_type).collect::<Vec<_>>().join(", "),
            display_type(&function.return_type)
        ),
        ValkyrieType::Tuple(items) => format!("({})", items.iter().map(display_type).collect::<Vec<_>>().join(", ")),
        ValkyrieType::LabeledTuple(fields) => {
            format!("({})", fields.iter().map(|field| format!("{}: {}", field.name, display_type(&field.ty))).collect::<Vec<_>>().join(", "))
        }
        ValkyrieType::Row(row) => format!(
            "{{ {} }}",
            row.methods
                .iter()
                .map(|method| {
                    format!(
                        "{}({}) -> {}",
                        method.name,
                        method.params.iter().map(display_type).collect::<Vec<_>>().join(", "),
                        display_type(&method.return_type)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ValkyrieType::Array(item) => format!("[{}]", display_type(item)),
        ValkyrieType::FixedArray { element, length } => {
            format!("[{}; {}]", display_type(element), length)
        }
        ValkyrieType::TypeLambda(lambda) => format!(
            "type lambda({}) -> {}",
            lambda.params.iter().map(|item| item.name.to_string()).collect::<Vec<_>>().join(", "),
            display_type(&lambda.body)
        ),
        ValkyrieType::TraitObject(object) => {
            format!("{}<{}>", object.trait_path, object.type_arguments.iter().map(display_type).collect::<Vec<_>>().join(", "))
        }
        ValkyrieType::Associated(associated) => {
            format!("{}::{}", display_type(&associated.base), associated.name)
        }
        ValkyrieType::AutoType => "auto".to_string(),
        ValkyrieType::SelfType => "Self".to_string(),
        ValkyrieType::Nullable(payload) => format!("{}?", display_type(payload)),
        ValkyrieType::Union(members) => members.iter().map(display_type).collect::<Vec<_>>().join(" | "),
        ValkyrieType::Intersection(members) => members.iter().map(display_type).collect::<Vec<_>>().join(" & "),
    }
}

pub fn integer_type_name(signed: bool, bits: u16) -> String {
    if signed { format!("i{bits}") } else { format!("u{bits}") }
}
