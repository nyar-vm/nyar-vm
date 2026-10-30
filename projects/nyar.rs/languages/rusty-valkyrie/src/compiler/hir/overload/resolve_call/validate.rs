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
        candidate_has_receiver_parameter, enrich_sum_variant_operation_facts, match_intrinsic_builtin_candidate,
        match_singleton_method_candidate, overload_symbol_path, peel_explicit_generic_arguments, try_resolve_constructor,
        try_resolve_constructor_call, try_resolve_instance_method, try_resolve_nullary_constructor, try_resolve_pattern_extractor,
        try_resolve_qualified_free_function, try_resolve_singleton_method, try_resolve_try_propagate_ok_variant,
        try_resolve_type_static_method,
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
pub fn validate_extractor_patterns(module: &HirModule) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    let candidates = collect_module_candidates(module);
    let type_relations = TypeRelationContext::from_module(module);
    // 与 resolve_hir_calls 共用唯一生产者；禁止再拼一份短名 BTreeMap。
    let struct_fields = collect_aggregate_field_map(module);
    for function in &module.functions {
        validate_function_extractor_patterns(function, &candidates, &type_relations, &struct_fields)?;
    }
    for item in &module.structs {
        for method in &item.methods {
            validate_function_extractor_patterns(method, &candidates, &type_relations, &struct_fields)?;
        }
    }
    Ok(())
}

fn validate_function_extractor_patterns(
    function: &HirFunction,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    struct_fields: &AggregateFieldIndex,
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    let mut locals = BTreeMap::new();
    for param in &function.params {
        locals.insert(param.name.name.to_string(), param.ty.clone());
    }
    validate_block_extractor_patterns(&function.body, candidates, type_relations, &locals, struct_fields)
}

fn validate_block_extractor_patterns(
    block: &HirBlock,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
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
                    validate_pattern_extractor_contract(pattern, inferred_type.as_ref(), candidates, type_relations, struct_fields)?;
                }
                if let Some(initializer) = initializer {
                    validate_expr_extractor_patterns(initializer, candidates, type_relations, &locals, struct_fields)?;
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
            HirStatementKind::Expr(expr) => validate_expr_extractor_patterns(expr, candidates, type_relations, &locals, struct_fields)?,
        }
    }
    if let Some(expr) = &block.expr {
        validate_expr_extractor_patterns(expr, candidates, type_relations, &locals, struct_fields)?;
    }
    Ok(())
}

fn validate_expr_extractor_patterns(
    expr: &HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    match &expr.kind {
        HirExprKind::Call { callee, args, .. } => {
            validate_expr_extractor_patterns(callee, candidates, type_relations, locals, struct_fields)?;
            for arg in args {
                validate_expr_extractor_patterns(&arg.value, candidates, type_relations, locals, struct_fields)?;
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
            validate_expr_extractor_patterns(callee, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::TryPropagate { inner, .. } => {
            validate_expr_extractor_patterns(inner, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::Cast { expr, .. } => {
            validate_expr_extractor_patterns(expr, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::StoreField { object, value, .. } => {
            validate_expr_extractor_patterns(object, candidates, type_relations, locals, struct_fields)?;
            validate_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::ArrayNew { length, .. } => validate_expr_extractor_patterns(length, candidates, type_relations, locals, struct_fields)?,
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } | HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                validate_expr_extractor_patterns(item, candidates, type_relations, locals, struct_fields)?;
            }
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                validate_expr_extractor_patterns(arg, candidates, type_relations, locals, struct_fields)?;
            }
        }
        HirExprKind::Lambda { body, .. } => validate_block_extractor_patterns(body, candidates, type_relations, locals, struct_fields)?,
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                validate_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields)?;
            }
            for method in methods {
                validate_function_extractor_patterns(method, candidates, type_relations, struct_fields)?;
            }
        }
        HirExprKind::Return(Some(value)) | HirExprKind::Yield(Some(value)) | HirExprKind::Resume(value) => {
            validate_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::Break { expr: Some(value), .. } => {
            validate_expr_extractor_patterns(value, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            validate_expr_extractor_patterns(scrutinee, candidates, type_relations, locals, struct_fields)?;
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, struct_fields, &BTreeSet::new());
            for arm in arms {
                validate_pattern_extractor_contract(&arm.pattern, scrutinee_type.as_ref(), candidates, type_relations, struct_fields)?;
                if let Some(guard) = &arm.guard {
                    validate_expr_extractor_patterns(guard, candidates, type_relations, locals, struct_fields)?;
                }
                validate_expr_extractor_patterns(&arm.body, candidates, type_relations, locals, struct_fields)?;
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            validate_expr_extractor_patterns(scrutinee, candidates, type_relations, locals, struct_fields)?;
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, struct_fields, &BTreeSet::new());
            validate_pattern_extractor_contract(pattern, scrutinee_type.as_ref(), candidates, type_relations, struct_fields)?;
            validate_block_extractor_patterns(then_branch, candidates, type_relations, locals, struct_fields)?;
            if let Some(else_branch) = else_branch {
                validate_block_extractor_patterns(else_branch, candidates, type_relations, locals, struct_fields)?;
            }
        }
        HirExprKind::Catch { expr, arms } => {
            validate_expr_extractor_patterns(expr, candidates, type_relations, locals, struct_fields)?;
            let scrutinee_type = infer_catch_scrutinee_type(expr, locals);
            for arm in arms {
                validate_pattern_extractor_contract(&arm.pattern, scrutinee_type.as_ref(), candidates, type_relations, struct_fields)?;
                if let Some(guard) = &arm.guard {
                    validate_expr_extractor_patterns(guard, candidates, type_relations, locals, struct_fields)?;
                }
                validate_expr_extractor_patterns(&arm.body, candidates, type_relations, locals, struct_fields)?;
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                validate_expr_extractor_patterns(iterator, candidates, type_relations, locals, struct_fields)?;
            }
            if let Some(condition) = condition {
                validate_expr_extractor_patterns(condition, candidates, type_relations, locals, struct_fields)?;
            }
            if let (Some(pattern), Some(iterator)) = (pattern, iterator) {
                let iter_type = infer_expr_type(iterator, locals);
                let element_type = iter_type.as_ref().map(|ty| match ty {
                    ValkyrieType::Array(inner) => inner.as_ref().clone(),
                    other => other.clone(),
                });
                validate_pattern_extractor_contract(pattern, element_type.as_ref(), candidates, type_relations, struct_fields)?;
            }
            validate_block_extractor_patterns(body, candidates, type_relations, locals, struct_fields)?;
        }
        HirExprKind::Block(block) => validate_block_extractor_patterns(block, candidates, type_relations, locals, struct_fields)?,
        HirExprKind::TryScope { body, .. } => validate_block_extractor_patterns(body, candidates, type_relations, locals, struct_fields)?,
        HirExprKind::If { condition, then_branch, else_branch } => {
            validate_expr_extractor_patterns(condition, candidates, type_relations, locals, struct_fields)?;
            validate_block_extractor_patterns(then_branch, candidates, type_relations, locals, struct_fields)?;
            if let Some(else_branch) = else_branch {
                validate_block_extractor_patterns(else_branch, candidates, type_relations, locals, struct_fields)?;
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
        | HirExprKind::SuperCall { .. }
        | HirExprKind::With { .. } => {}
    }
    Ok(())
}

fn validate_pattern_extractor_contract(
    pattern: &HirPattern,
    scrutinee_type: Option<&ValkyrieType>,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    struct_fields: &AggregateFieldIndex,
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    match pattern {
        HirPattern::Extractor(extractor) => match extractor {
            HirExtractorPattern::Constructor { canonical_callee, resolved, fields, .. } => {
                if resolved.is_none() {
                    if let Some(actual_type) = scrutinee_type {
                        if let Some(error) = diagnose_pattern_extractor_failure(canonical_callee, actual_type, candidates, type_relations) {
                            return Err(error);
                        }
                    }
                    else if !is_builtin_pattern_extractor(canonical_callee) {
                        if let Some(error) = diagnose_unresolved_extractor_without_scrutinee_type(canonical_callee, candidates) {
                            return Err(error);
                        }
                    }
                }
                let field_scrutinee_types = resolved
                    .as_ref()
                    .and_then(|resolved| resolved.extractor_payload_type.as_ref())
                    .map(|payload| payload_field_scrutinee_types(payload, struct_fields))
                    .unwrap_or_default();
                for (index, field) in fields.iter().enumerate() {
                    let field_scrutinee = field_scrutinee_types.get(index).and_then(|ty| ty.as_ref());
                    validate_pattern_extractor_contract(field, field_scrutinee, candidates, type_relations, struct_fields)?;
                }
            }
            HirExtractorPattern::Array { canonical_callee, resolved, prefix, suffix, .. } => {
                if resolved.is_none() {
                    if let Some(actual_type) = scrutinee_type {
                        if let Some(error) = diagnose_pattern_extractor_failure(canonical_callee, actual_type, candidates, type_relations) {
                            return Err(error);
                        }
                    }
                    else if !is_builtin_pattern_extractor(canonical_callee) {
                        if let Some(error) = diagnose_unresolved_extractor_without_scrutinee_type(canonical_callee, candidates) {
                            return Err(error);
                        }
                    }
                }
                for item in prefix.iter().chain(suffix.iter()) {
                    validate_pattern_extractor_contract(item, None, candidates, type_relations, struct_fields)?;
                }
            }
        },
        HirPattern::Tuple(items) | HirPattern::Or(items) => {
            for item in items {
                validate_pattern_extractor_contract(item, scrutinee_type, candidates, type_relations, struct_fields)?;
            }
        }
        HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
            validate_pattern_extractor_contract(pattern, scrutinee_type, candidates, type_relations, struct_fields)?;
        }
        HirPattern::Object { name, fields, .. } => {
            for (field_name, field_pattern) in fields {
                let field_scrutinee = object_field_scrutinee_type(name.as_ref(), field_name, struct_fields);
                validate_pattern_extractor_contract(field_pattern, field_scrutinee, candidates, type_relations, struct_fields)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn is_builtin_pattern_extractor(canonical_callee: &NamePath) -> bool {
    matches!(
        canonical_callee.parts(),
        [owner, method] if owner.as_str() == "array" && method.as_str() == "extractor"
    )
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
