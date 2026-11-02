//! Pattern extractor validation (runs after `resolve_hir_calls`).

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::{
        try_propagate::{option_apply_payload_type, result_variant_payload_type},
        type_relation::TypeRelationContext,
    },
    mir::{AggregateFieldIndex, collect_aggregate_field_map},
    types::{
        Identifier, NamePath,
        hir::{
            HirBlock, HirCallArgument, HirCallableDomain, HirExpr, HirExprKind, HirExtractorPattern, HirFunction, HirIdentifier, HirMatchArm,
            HirModule, HirParam, HirPattern, HirResolvedCall, HirStatement, HirStatementKind, HirTrait, HirVariant, PatternRefutability,
            ValkyrieType,
        },
    },
};

use crate::compiler::hir::overload::candidates::collect_module_candidates;

use crate::compiler::hir::overload::{
    call_resolver::{
        candidate_has_receiver_parameter, enrich_sum_variant_operation_facts, match_singleton_method_candidate, overload_symbol_path,
        peel_explicit_generic_arguments, try_resolve_constructor, try_resolve_constructor_call, try_resolve_instance_method,
        try_resolve_nullary_constructor, try_resolve_pattern_extractor, try_resolve_qualified_free_function, try_resolve_singleton_method,
        try_resolve_try_propagate_ok_variant, try_resolve_type_static_method,
    },
    diagnostics::{OverloadCandidate, OverloadResolutionError},
    expr_position::ExprPosition,
    symbol_match::aggregate_constructor_matches_name,
    types::{
        canonical_extractor_callee, compute_call_match_kind, field_identity_from_type, infer_catch_scrutinee_type, infer_expr_type,
        infer_scrutinee_type, lookup_struct_field_type, normalize_local_field_chain, object_field_scrutinee_type,
        payload_field_scrutinee_types, should_resolve_name_pattern_as_type,
    },
};

use super::{
    pattern_policy::{
        diagnose_pattern_extractor_failure, diagnose_unresolved_extractor_without_scrutinee_type, pattern_extractor_receiver_type,
    },
    resolve::bind_pattern_type,
};

/// 校验模块内 pattern extractor 合同（在 `resolve_hir_calls` 之后调用）。
///
/// 同一编译单元内全部 extractor 失败在此收集并记入诊断暂存汇（多诊断报告）；
/// 控制流保持返回首个错误，fail-closed 不变。单个 pattern 失败即跳过其子树：
/// 子 pattern 的 scrutinee 类型由父 extractor 载荷派生，父失败不再级联出
/// 「无 scrutinee 类型」的次生错误。
///
/// 可达性说明：数组 extractor（`HirExtractorPattern::Array`）属计划内语法
/// （`case [head, ...tail]`），等待 oak AST/parser 节点与 lowering 构造接入；
/// 上游接入后本收集路径即激活，与调用合同收集同构。
pub fn validate_extractor_patterns(module: &HirModule) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    let candidates = collect_module_candidates(module);
    let type_relations = TypeRelationContext::from_module(module);
    // 与 resolve_hir_calls 共用唯一生产者；禁止再拼一份短名 BTreeMap。
    let struct_fields = collect_aggregate_field_map(module);
    let mut errors = Vec::new();
    for function in &module.functions {
        collect_function_extractor_patterns(function, &candidates, &type_relations, &struct_fields, &mut errors);
    }
    for item in &module.structs {
        for method in &item.methods {
            collect_function_extractor_patterns(method, &candidates, &type_relations, &struct_fields, &mut errors);
        }
    }
    let mut errors = errors.into_iter();
    let Some(first) = errors.next()
    else {
        return Ok(());
    };
    let source = extractor_diagnostic_source(module);
    for error in std::iter::once(&first).chain(errors.as_slice()) {
        crate::compiler::diagnostics::pending::push_compile_diagnostic(compile_diagnostic_from_parse_error(error, &source));
    }
    Err(first)
}

/// extractor 失败的源码身份：优先模块 `source_path`，无文件身份时回落到稳定标签。
fn extractor_diagnostic_source(module: &HirModule) -> nyar_types::DiagnosticSource {
    if module.source_path.trim().is_empty() {
        nyar_types::DiagnosticSource { id: "<frontend>".into(), revision: None }
    }
    else {
        nyar_types::DiagnosticSource { id: module.source_path.clone(), revision: None }
    }
}

/// 把 extractor 校验生产的 `ParseError` 投影为多诊断报告条目。
///
/// `code` 复用 `HirError::FrontendRejected` 的唯一事实源（不重拼字符串）；
/// span 取自 `ParseError::Invalid` 携带的源码区间，没有区间时为空 span。
fn compile_diagnostic_from_parse_error(
    error: &crate::compiler::syntax::oak::ParseError,
    source: &nyar_types::DiagnosticSource,
) -> crate::compiler::diagnostics::pending::CompileDiagnostic {
    use nyar_types::{DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSpan};
    let span = match error {
        crate::compiler::syntax::oak::ParseError::Invalid { span: Some(range), .. } => DiagnosticSpan { start: range.start, end: range.end },
        _ => DiagnosticSpan { start: 0, end: 0 },
    };
    let code = crate::compiler::hir::lowering::errors::HirError::from(crate::compiler::syntax::oak::ParseError::Invalid {
        message: String::new(),
        span: None,
    })
    .code();
    let row = DiagnosticRow::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticPhase::Hir,
        DiagnosticOwner::frontend("rusty-valkyrie"),
        source.clone(),
        span,
    )
    .with_cause(error.to_string());
    crate::compiler::diagnostics::pending::CompileDiagnostic::new(row, error.to_string())
}

fn collect_function_extractor_patterns(
    function: &HirFunction,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    struct_fields: &AggregateFieldIndex,
    errors: &mut Vec<crate::compiler::syntax::oak::ParseError>,
) {
    let mut locals = BTreeMap::new();
    for param in &function.params {
        locals.insert(param.name.name.to_string(), param.ty.clone());
    }
    collect_block_extractor_patterns(&function.body, candidates, type_relations, &locals, struct_fields, errors)
}

fn collect_block_extractor_patterns(
    block: &HirBlock,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    errors: &mut Vec<crate::compiler::syntax::oak::ParseError>,
) {
    let mut locals = locals.clone();
    for statement in &block.statements {
        match &statement.kind {
            HirStatementKind::Let { pattern, ty, initializer, .. } => {
                // 对于 refutable pattern（如 `let Some(x) = opt`），跳过 extractor 校验，
                // 由后续 `check_pattern_refutability` 统一拒绝。
                if pattern.refutability() != PatternRefutability::Refutable {
                    let inferred_type = ty
                        .as_ref()
                        .cloned()
                        .or_else(|| initializer.as_ref().and_then(|init| infer_scrutinee_type(init, &locals, struct_fields, &BTreeSet::new())));
                    collect_pattern_extractor_contract(pattern, inferred_type.as_ref(), candidates, type_relations, struct_fields, errors);
                }
                if let Some(initializer) = initializer {
                    collect_expr_extractor_patterns(initializer, candidates, type_relations, &locals, struct_fields, errors);
                }
                if let Some(ty) = ty {
                    bind_pattern_type(pattern, ty, &mut locals);
                }
                else if let Some(initializer) = initializer {
                    if let Some(binding_type) = infer_scrutinee_type(initializer, &locals, struct_fields, &BTreeSet::new()) {
                        bind_pattern_type(pattern, &binding_type, &mut locals);
                    }
                }
            }
            HirStatementKind::Expr(expr) => collect_expr_extractor_patterns(expr, candidates, type_relations, &locals, struct_fields, errors),
        }
    }
    if let Some(expr) = &block.expr {
        collect_expr_extractor_patterns(expr, candidates, type_relations, &locals, struct_fields, errors);
    }
}

fn collect_expr_extractor_patterns(
    expr: &HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    errors: &mut Vec<crate::compiler::syntax::oak::ParseError>,
) {
    match &expr.kind {
        HirExprKind::Call { callee, args, .. } => {
            collect_expr_extractor_patterns(callee, candidates, type_relations, locals, struct_fields, errors);
            for arg in args {
                collect_expr_extractor_patterns(&arg.value, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::GenericApply { callee, .. }
        | HirExprKind::FieldInit { value: callee, .. }
        | HirExprKind::Await(callee)
        | HirExprKind::Awake(callee)
        | HirExprKind::BlockOn(callee)
        | HirExprKind::YieldFrom(callee)
        | HirExprKind::Raise(callee)
        | HirExprKind::Resume(callee)
        | HirExprKind::Assign { value: callee, .. }
        | HirExprKind::FieldAccess { object: callee, .. } => {
            collect_expr_extractor_patterns(callee, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::TryPropagate { inner, .. } => {
            collect_expr_extractor_patterns(inner, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::Cast { expr, .. } => {
            collect_expr_extractor_patterns(expr, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::StoreField { object, value, .. } => {
            collect_expr_extractor_patterns(object, candidates, type_relations, locals, struct_fields, errors);
            collect_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::ArrayNew { length, .. } => {
            collect_expr_extractor_patterns(length, candidates, type_relations, locals, struct_fields, errors)
        }
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } | HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                collect_expr_extractor_patterns(item, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                collect_expr_extractor_patterns(arg, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::Lambda { body, .. } => collect_block_extractor_patterns(body, candidates, type_relations, locals, struct_fields, errors),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                collect_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields, errors);
            }
            for method in methods {
                collect_function_extractor_patterns(method, candidates, type_relations, struct_fields, errors);
            }
        }
        HirExprKind::Return(Some(value)) | HirExprKind::Yield(Some(value)) | HirExprKind::Resume(value) => {
            collect_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::Break { expr: Some(value), .. } => {
            collect_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            collect_expr_extractor_patterns(scrutinee, candidates, type_relations, locals, struct_fields, errors);
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, struct_fields, &BTreeSet::new());
            for arm in arms {
                collect_pattern_extractor_contract(&arm.pattern, scrutinee_type.as_ref(), candidates, type_relations, struct_fields, errors);
                if let Some(guard) = &arm.guard {
                    collect_expr_extractor_patterns(guard, candidates, type_relations, locals, struct_fields, errors);
                }
                collect_expr_extractor_patterns(&arm.body, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            collect_expr_extractor_patterns(scrutinee, candidates, type_relations, locals, struct_fields, errors);
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, struct_fields, &BTreeSet::new());
            collect_pattern_extractor_contract(pattern, scrutinee_type.as_ref(), candidates, type_relations, struct_fields, errors);
            collect_block_extractor_patterns(then_branch, candidates, type_relations, locals, struct_fields, errors);
            if let Some(else_branch) = else_branch {
                collect_block_extractor_patterns(else_branch, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::Catch { expr, arms } => {
            collect_expr_extractor_patterns(expr, candidates, type_relations, locals, struct_fields, errors);
            let scrutinee_type = infer_catch_scrutinee_type(expr, locals);
            for arm in arms {
                collect_pattern_extractor_contract(&arm.pattern, scrutinee_type.as_ref(), candidates, type_relations, struct_fields, errors);
                if let Some(guard) = &arm.guard {
                    collect_expr_extractor_patterns(guard, candidates, type_relations, locals, struct_fields, errors);
                }
                collect_expr_extractor_patterns(&arm.body, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                collect_expr_extractor_patterns(iterator, candidates, type_relations, locals, struct_fields, errors);
            }
            if let Some(condition) = condition {
                collect_expr_extractor_patterns(condition, candidates, type_relations, locals, struct_fields, errors);
            }
            if let (Some(pattern), Some(iterator)) = (pattern, iterator) {
                let iter_type = infer_expr_type(iterator, locals);
                let element_type = iter_type.as_ref().map(|ty| match ty {
                    ValkyrieType::Array(inner) => inner.as_ref().clone(),
                    other => other.clone(),
                });
                collect_pattern_extractor_contract(pattern, element_type.as_ref(), candidates, type_relations, struct_fields, errors);
            }
            collect_block_extractor_patterns(body, candidates, type_relations, locals, struct_fields, errors);
        }
        HirExprKind::Block(block) => collect_block_extractor_patterns(block, candidates, type_relations, locals, struct_fields, errors),
        HirExprKind::TryScope { body, .. } => collect_block_extractor_patterns(body, candidates, type_relations, locals, struct_fields, errors),
        HirExprKind::If { condition, then_branch, else_branch } => {
            collect_expr_extractor_patterns(condition, candidates, type_relations, locals, struct_fields, errors);
            collect_block_extractor_patterns(then_branch, candidates, type_relations, locals, struct_fields, errors);
            if let Some(else_branch) = else_branch {
                collect_block_extractor_patterns(else_branch, candidates, type_relations, locals, struct_fields, errors);
            }
        }
        HirExprKind::Return(None)
        | HirExprKind::Break { expr: None, .. }
        | HirExprKind::Continue { .. }
        | HirExprKind::Yield(None)
        | HirExprKind::Fallthrough
        | HirExprKind::Literal(_)
        | HirExprKind::Variable(_)
        | HirExprKind::Path(_)
        | HirExprKind::FunctionReference(_)
        | HirExprKind::SuperCall { .. }
        | HirExprKind::With { .. } => {}
    }
}

/// 收集单个 pattern 的 extractor 失败；失败即返回、不再递归子 pattern（去级联）。
fn collect_pattern_extractor_contract(
    pattern: &HirPattern,
    scrutinee_type: Option<&ValkyrieType>,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    struct_fields: &AggregateFieldIndex,
    errors: &mut Vec<crate::compiler::syntax::oak::ParseError>,
) {
    match pattern {
        HirPattern::Extractor(extractor) => match extractor {
            HirExtractorPattern::Constructor { canonical_callee, resolved, fields, .. } => {
                if resolved.is_none() {
                    if let Some(actual_type) = scrutinee_type {
                        if let Some(error) = diagnose_pattern_extractor_failure(canonical_callee, actual_type, candidates, type_relations) {
                            errors.push(error);
                            return;
                        }
                    }
                    else if let Some(error) = diagnose_unresolved_extractor_without_scrutinee_type(canonical_callee, candidates) {
                        errors.push(error);
                        return;
                    }
                }
                let field_scrutinee_types = resolved
                    .as_ref()
                    .and_then(|resolved| resolved.extractor_payload_type.as_ref())
                    .map(|payload| payload_field_scrutinee_types(payload, struct_fields))
                    .unwrap_or_default();
                for (index, field) in fields.iter().enumerate() {
                    let field_scrutinee = field_scrutinee_types.get(index).and_then(|ty| ty.as_ref());
                    collect_pattern_extractor_contract(field, field_scrutinee, candidates, type_relations, struct_fields, errors);
                }
            }
            HirExtractorPattern::Array { canonical_callee, resolved, prefix, suffix, .. } => {
                if resolved.is_none() {
                    if let Some(actual_type) = scrutinee_type {
                        if let Some(error) = diagnose_pattern_extractor_failure(canonical_callee, actual_type, candidates, type_relations) {
                            errors.push(error);
                            return;
                        }
                    }
                    else if let Some(error) = diagnose_unresolved_extractor_without_scrutinee_type(canonical_callee, candidates) {
                        errors.push(error);
                        return;
                    }
                }
                for item in prefix.iter().chain(suffix.iter()) {
                    collect_pattern_extractor_contract(item, None, candidates, type_relations, struct_fields, errors);
                }
            }
        },
        HirPattern::Tuple(items) | HirPattern::Or(items) => {
            for item in items {
                collect_pattern_extractor_contract(item, scrutinee_type, candidates, type_relations, struct_fields, errors);
            }
        }
        HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
            collect_pattern_extractor_contract(pattern, scrutinee_type, candidates, type_relations, struct_fields, errors);
        }
        HirPattern::Object { name, fields, .. } => {
            for (field_name, field_pattern) in fields {
                let field_scrutinee = object_field_scrutinee_type(name.as_ref(), field_name, struct_fields);
                collect_pattern_extractor_contract(field_pattern, field_scrutinee, candidates, type_relations, struct_fields, errors);
            }
        }
        _ => {}
    }
}

pub(super) fn is_option_owner_type(ty: &ValkyrieType) -> bool {
    crate::compiler::hir::try_propagate::is_option_apply_type(ty)
}

pub(super) fn is_result_owner_type(ty: &ValkyrieType) -> bool {
    crate::compiler::hir::try_propagate::is_result_apply_type(ty)
}

fn bind_unite_extractor_fields(fields: &[HirPattern], ty: &ValkyrieType, variant: Option<&str>, locals: &mut BTreeMap<String, ValkyrieType>) {
    if let Some(payload) = option_apply_payload_type(ty) {
        for (index, field) in fields.iter().enumerate() {
            let field_ty = if index == 0 { Some(payload.clone()) } else { None };
            if let Some(field_ty) = field_ty {
                bind_pattern_type(field, &field_ty, locals);
            }
        }
        return;
    }
    if let Some(variant) = variant {
        if let Some(payload) = result_variant_payload_type(ty, variant) {
            for (index, field) in fields.iter().enumerate() {
                let field_ty = if index == 0 { Some(payload.clone()) } else { None };
                if let Some(field_ty) = field_ty {
                    bind_pattern_type(field, &field_ty, locals);
                }
            }
            return;
        }
    }
    if is_option_owner_type(ty) || is_result_owner_type(ty) {
        for field in fields {
            bind_pattern_type(field, &ValkyrieType::AutoType, locals);
        }
    }
}
