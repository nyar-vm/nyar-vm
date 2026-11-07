//! 调用解析失败解释：在解析失败现场给出结构化原因。
//!
//! 只使用解析器在同一现场可观察的事实（候选集、局部类型、类型关系），
//! 不虚构不可观察的中间状态；分类标签与文案归 `HirCallResolutionFailureKind`。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::{
        control_flow_validation::display_type,
        overload::{
            call_resolver::core::{candidate_has_receiver_parameter, peel_explicit_generic_arguments},
            diagnostics::{OverloadCandidate, OverloadDomain},
            substitution::has_unresolved_generic_arguments,
            symbol_match::{
                candidate_owned_method_matches, candidate_trait_method_matches, symbol_matches_imported_dependency_export,
                symbol_matches_unqualified_free_call,
            },
            types::{infer_expr_type, infer_scrutinee_type},
        },
        type_relation::TypeRelationContext,
    },
    mir::AggregateFieldIndex,
    types::{
        Identifier,
        hir::{HirCallArgument, HirCallResolutionFailureKind, HirExpr, HirExprKind, HirKind, ValkyrieType},
    },
};

/// 在 `try_resolve_call` 返回 `None` 的同一现场推导失败原因分类。
pub(crate) fn explain_unresolved_call(
    callee: &HirExpr,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> HirCallResolutionFailureKind {
    let (explicit_generic_arguments, peeled) = peel_explicit_generic_arguments(callee);
    if explicit_generic_arguments.iter().any(has_unresolved_generic_arguments) {
        return HirCallResolutionFailureKind::GenericsUnresolved;
    }
    if let Some(mismatch) = explicit_generic_argument_mismatch(peeled, &explicit_generic_arguments, candidates, type_relations) {
        return mismatch;
    }
    match &peeled.kind {
        HirExprKind::FieldAccess { object, field, .. } => {
            let receiver =
                infer_expr_type(object, locals).or_else(|| infer_scrutinee_type(object, locals, struct_fields, singleton_names));
            // 同名候选集与接收者推断无关：接收者未定时同样要点名，便于区分
            // 「没有该成员」与「有成员但接收者身份未定」。
            let named = candidates
                .iter()
                .filter(|candidate| {
                    matches!(candidate.domain, OverloadDomain::Function)
                        && candidate_has_receiver_parameter(candidate)
                        && (candidate_owned_method_matches(candidate, field) || candidate_trait_method_matches(candidate, field))
                })
                .count();
            match receiver {
                None => HirCallResolutionFailureKind::ReceiverTypeUnknown { member: field.to_string(), candidates: named },
                Some(receiver_type) => {
                    if named == 0 {
                        HirCallResolutionFailureKind::NoMethodCandidate { member: field.to_string(), receiver: display_type(&receiver_type) }
                    }
                    else {
                        HirCallResolutionFailureKind::NoViableOverload { candidates: named }
                    }
                }
            }
        }
        HirExprKind::Variable(identifier) => {
            name_based_explanation(&identifier.name, args, candidates, type_relations, locals, struct_fields, singleton_names)
        }
        HirExprKind::Path(path) => match path.parts().last() {
            Some(name) => name_based_explanation(name, args, candidates, type_relations, locals, struct_fields, singleton_names),
            None => HirCallResolutionFailureKind::NameNotFound,
        },
        _ => HirCallResolutionFailureKind::ComputedCallee,
    }
}

/// 显式泛型实参数量与声明 binder 数量的结构化不符。
///
/// 只在该调用名下的全部候选 binder 数量一致时成立：候选池内部数量不一致意味着
/// 失败还牵扯重载选择，交给常规「同名候选无一存活」，不在这里猜期望值。
/// 期望种类只取自声明处 `GenericType`；未登记种类的 binder 不虚构种类。
fn explicit_generic_argument_mismatch(
    peeled: &HirExpr,
    explicit: &[ValkyrieType],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
) -> Option<HirCallResolutionFailureKind> {
    if explicit.is_empty() {
        return None;
    }
    let named = candidates
        .iter()
        .filter(|candidate| matches!(candidate.domain, OverloadDomain::Function))
        .filter(|candidate| candidate_matches_callee_name(candidate, peeled, type_relations))
        .collect::<Vec<_>>();
    let first = named.first()?;
    let expected_binders = first.generic_binders.len();
    if named.iter().any(|candidate| candidate.generic_binders.len() != expected_binders) {
        return None;
    }
    if expected_binders == explicit.len() {
        return None;
    }
    let expected_kinds = first
        .generic_binders
        .iter()
        .filter_map(|binder| first.generic_binder_kinds.get(binder).map(binder_kind_label))
        .collect();
    Some(HirCallResolutionFailureKind::ExplicitGenericArgumentsMismatch {
        expected_binders,
        expected_kinds,
        actual_arguments: explicit.iter().map(display_type).collect(),
    })
}

/// 候选是否以被调用者的表面名注册。
///
/// 成员访问只认同名字段槽（自有 impl 或 trait 方法）；限定路径按完整符号身份
/// 对齐，不用末段回退；单段名字走与自由函数解析同一套可见性谓词。
fn candidate_matches_callee_name(candidate: &OverloadCandidate, peeled: &HirExpr, type_relations: &TypeRelationContext) -> bool {
    match &peeled.kind {
        HirExprKind::FieldAccess { field, .. } => {
            candidate_owned_method_matches(candidate, field) || candidate_trait_method_matches(candidate, field)
        }
        HirExprKind::Variable(identifier) => named_free_candidate_matches(candidate, &identifier.name, type_relations),
        HirExprKind::Path(path) => match path.parts() {
            [name] => named_free_candidate_matches(candidate, name, type_relations),
            parts => candidate.symbol.parts() == parts,
        },
        _ => false,
    }
}

fn named_free_candidate_matches(candidate: &OverloadCandidate, name: &Identifier, type_relations: &TypeRelationContext) -> bool {
    let enclosing_module = type_relations.enclosing_module();
    candidate_owned_method_matches(candidate, name)
        || candidate_trait_method_matches(candidate, name)
        || symbol_matches_unqualified_free_call(&candidate.symbol, name, enclosing_module)
        || symbol_matches_imported_dependency_export(candidate, name)
        || type_relations.symbol_matches_import(&candidate.symbol, name)
        || type_relations.symbol_matches_group_scope(&candidate.symbol, name)
}

/// binder 种类的用户可读标签，供显式泛型实参诊断列出期望种类。
fn binder_kind_label(kind: &HirKind) -> String {
    match kind {
        HirKind::Type => "type".to_string(),
        HirKind::Function(domain, codomain) => format!("function({} -> {})", binder_kind_label(domain), binder_kind_label(codomain)),
    }
}

/// 按名字计数可见候选：零即名字未找到，否则是同名候选无一存活。
///
/// 比较算子（`infix ==/!=/</>/<=/>=`）失败时先判标量族混比：
/// `char`/`u8` 一侧与整型一侧直接比较没有隐式统一，提示显式转换。
fn name_based_explanation(
    name: &Identifier,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> HirCallResolutionFailureKind {
    if is_comparison_operator(name.as_str())
        && let Some(mixed) = mixed_scalar_operands(args, locals, struct_fields, singleton_names)
    {
        return mixed;
    }
    let named = candidates
        .iter()
        .filter(|candidate| matches!(candidate.domain, OverloadDomain::Function))
        .filter(|candidate| named_free_candidate_matches(candidate, name, type_relations))
        .count();
    if named == 0 { HirCallResolutionFailureKind::NameNotFound } else { HirCallResolutionFailureKind::NoViableOverload { candidates: named } }
}

fn is_comparison_operator(name: &str) -> bool {
    matches!(name, "infix ==" | "infix !=" | "infix <" | "infix >" | "infix <=" | "infix >=")
}

/// 标量族：`char`/`u8` 是文本码元族，与整型族之间不做隐式统一。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarFamily {
    Text,
    Integer,
}

fn scalar_family(ty: &ValkyrieType) -> Option<ScalarFamily> {
    match ty {
        ValkyrieType::Character => Some(ScalarFamily::Text),
        ValkyrieType::Integer8 { signed: false } => Some(ScalarFamily::Text),
        ValkyrieType::Integer8 { signed: true }
        | ValkyrieType::Integer16 { .. }
        | ValkyrieType::Integer32 { .. }
        | ValkyrieType::Integer64 { .. }
        | ValkyrieType::Integer128 { .. } => Some(ScalarFamily::Integer),
        ValkyrieType::Named(name) => scalar_family_of_name(name.as_str()),
        _ => None,
    }
}

fn scalar_family_of_name(name: &str) -> Option<ScalarFamily> {
    match name {
        "char" | "u8" | "byte" => Some(ScalarFamily::Text),
        "i8" | "i16" | "i32" | "i64" | "i128" | "u16" | "u32" | "u64" | "u128" | "int" | "Int" => Some(ScalarFamily::Integer),
        _ => None,
    }
}

/// 比较两端的静态类型分属文本码元族与整型族时给出混比事实。
fn mixed_scalar_operands(
    args: &[HirCallArgument],
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<HirCallResolutionFailureKind> {
    let (left, right) = (args.first()?, args.get(1)?);
    let left_type = infer_scrutinee_type(&left.value, locals, struct_fields, singleton_names)?;
    let right_type = infer_scrutinee_type(&right.value, locals, struct_fields, singleton_names)?;
    let left_family = scalar_family(&left_type)?;
    let right_family = scalar_family(&right_type)?;
    if left_family == right_family {
        return None;
    }
    Some(HirCallResolutionFailureKind::MixedScalarComparison { left: display_type(&left_type), right: display_type(&right_type) })
}
