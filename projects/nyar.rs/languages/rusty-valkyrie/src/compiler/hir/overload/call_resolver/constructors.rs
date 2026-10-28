//! 调用解析核心：try_resolve_call / match_call_candidate 等。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::{
        call_binding::bind_call_arguments,
        construct_binding::bind_construct_fields,
        try_propagate::{
            is_option_apply_type, is_result_apply_type, nullable_payload_type, option_apply_payload_type, result_variant_payload_type,
        },
        type_relation::{ParameterMatchResult, TypeRelationContext},
    },
    mir::AggregateFieldIndex,
    semantic::operator_members,
    types::{
        Identifier, NamePath,
        hir::{
            HirCallArgument, HirCallableDomain, HirEnum, HirExpr, HirExprKind, HirFunction, HirLiteral, HirParam, HirPattern, HirResolvedCall,
            HirStringSegment, HirTrait, HirTraitBound, HirVariant, HirWhereConstraint, ValkyrieType,
        },
    },
};

use crate::compiler::hir::overload::{
    diagnostics::{OverloadCandidate, OverloadDomain, OverloadMatchKind, OverloadResolutionError},
    resolve_call::pattern_extractor_receiver_type,
    substitution::{
        extract_singleton_type_name, instantiate_trait_bound_candidate, instantiate_trait_method_candidate, substitute_self_type,
        substitute_type_parameters, substitute_type_self, substitute_type_vars, unify_call_type_binders,
    },
    symbol_match::{
        aggregate_constructor_matches_name, candidate_extractor_path_matches, candidate_owned_method_matches, candidate_trait_method_matches,
        candidate_variant_constructor_matches, symbol_matches_imported_dependency_export, symbol_matches_language_builtin,
        symbol_matches_unqualified_free_call,
    },
    r#trait::impl_nominal_type_name,
    types::{
        accepts_array_shape, compute_call_match_kind, extract_callable_name, infer_expr_type, infer_scrutinee_type, is_empty_array_literal,
    },
};

use super::{
    core::{candidate_has_receiver_parameter, match_call_candidate, overload_symbol_path, peel_explicit_generic_arguments},
    helpers::{enrich_unite_variant_constructor_facts, instantiate_candidate_for_context, try_resolve_constructor},
    patterns::enrich_sum_variant_operation_facts,
};

/// Constructor domain 专属解析路径。
pub(crate) fn try_resolve_constructor_call(
    callee: &HirExpr,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> Option<HirResolvedCall> {
    let (explicit_generic_arguments, callee) = peel_explicit_generic_arguments(callee);
    let callee_name = extract_callable_name(callee)?;
    let callee_path = NamePath::new(vec![callee_name.clone()]);

    let owned_variant_constructors = candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Constructor && candidate.owner.is_some())
        .filter(|candidate| candidate_variant_constructor_matches(candidate, &callee_name))
        .filter_map(|candidate| {
            if let Some(context) = contextual_type
                && let Some(instantiated) = instantiate_candidate_for_context(candidate, context)
                && let Some(matched) = match_call_candidate(
                    &instantiated,
                    args,
                    &explicit_generic_arguments,
                    type_relations,
                    locals,
                    struct_fields,
                    singleton_names,
                )
            {
                return Some(matched);
            }
            match_call_candidate(candidate, args, &explicit_generic_arguments, type_relations, locals, struct_fields, singleton_names)
        })
        .collect::<Vec<_>>();
    if let Ok(resolved) = crate::compiler::hir::overload::diagnostics::resolve_overload(&owned_variant_constructors) {
        return Some(build_constructor_resolved_call(
            &callee,
            args,
            resolved,
            contextual_type,
            candidates,
            locals,
            struct_fields,
            singleton_names,
        ));
    }

    let free_constructors = candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Constructor && candidate.owner.is_none())
        .filter(|candidate| aggregate_constructor_matches_name(candidate, &callee_path))
        .filter_map(|candidate| {
            if let Some(context) = contextual_type
                && let Some(instantiated) = instantiate_candidate_for_context(candidate, context)
                && let Some(matched) = match_call_candidate(
                    &instantiated,
                    args,
                    &explicit_generic_arguments,
                    type_relations,
                    locals,
                    struct_fields,
                    singleton_names,
                )
            {
                return Some(matched);
            }
            match_call_candidate(candidate, args, &explicit_generic_arguments, type_relations, locals, struct_fields, singleton_names)
        })
        .collect::<Vec<_>>();
    if let Ok(resolved) = crate::compiler::hir::overload::diagnostics::resolve_overload(&free_constructors) {
        return Some(build_constructor_resolved_call(
            &callee,
            args,
            resolved,
            contextual_type,
            candidates,
            locals,
            struct_fields,
            singleton_names,
        ));
    }

    None
}

pub(crate) fn build_constructor_resolved_call(
    callee: &HirExpr,
    args: &[HirCallArgument],
    resolved: OverloadCandidate,
    contextual_type: Option<&ValkyrieType>,
    candidates: &[OverloadCandidate],
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> HirResolvedCall {
    let has_receiver = candidate_has_receiver_parameter(&resolved);
    let symbol = overload_symbol_path(&resolved);
    let return_type = resolved.signature.return_type;
    let mut call = HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol,
        domain: HirCallableDomain::Constructor,
        return_type,
        parameter_types: resolved.signature.params,
        has_receiver,
        extractor_payload_type: None,
    };
    enrich_unite_variant_constructor_facts(callee, args, &mut call, contextual_type, candidates, locals, struct_fields, singleton_names);
    call
}

pub(crate) fn try_resolve_qualified_free_function(
    path: &NamePath,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<HirResolvedCall> {
    let filtered = candidates
        .iter()
        .filter(|candidate| {
            if candidate.owner.is_some() {
                return false;
            }
            candidate.symbol.parts() == path.parts()
        })
        .filter_map(|candidate| match_call_candidate(candidate, args, &[], type_relations, locals, struct_fields, singleton_names))
        .collect::<Vec<_>>();
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    let domain = match resolved.domain {
        OverloadDomain::Function => HirCallableDomain::Function,
        OverloadDomain::Constructor => HirCallableDomain::Constructor,
        OverloadDomain::Extractor => HirCallableDomain::Extractor,
    };
    Some(HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol: resolved.symbol,
        domain,
        return_type: resolved.signature.return_type,
        parameter_types: resolved.signature.params,
        has_receiver: false,
        extractor_payload_type: None,
    })
}

/// 未解析的无参变体构造；外部测试入口。
#[doc(hidden)]
pub fn try_resolve_nullary_constructor(
    name: &Identifier,
    contextual_type: Option<&ValkyrieType>,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<HirResolvedCall> {
    if singleton_names.contains(name) || locals.contains_key(name.as_str()) {
        return None;
    }
    let narrowed = candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Constructor)
        .filter(|candidate| candidate.signature.params.is_empty())
        .filter(|candidate| candidate_variant_constructor_matches(candidate, name))
        .filter_map(|candidate| {
            let Some(context) = contextual_type
            else {
                return Some(candidate.clone());
            };
            let mut substitutions = BTreeMap::new();
            unify_call_type_binders(&candidate.signature.return_type, context, &candidate.generic_binders, &mut substitutions)?;
            let instantiated = substitute_type_vars(&candidate.signature.return_type, &substitutions);
            if matches!(type_relations.match_parameter(&instantiated, context), ParameterMatchResult::NoMatch { .. }) {
                return None;
            }
            let mut candidate = candidate.clone();
            candidate.signature.return_type = instantiated;
            Some(candidate)
        })
        .collect::<Vec<_>>();
    if narrowed.is_empty() {
        return None;
    }
    try_resolve_constructor(
        &NamePath::new(vec![name.clone()]),
        &[],
        &narrowed,
        type_relations,
        locals,
        struct_fields,
        singleton_names,
        contextual_type,
    )
}

/// singleton 方法匹配；外部测试入口。
#[doc(hidden)]
pub fn match_singleton_method_candidate(
    candidate: &OverloadCandidate,
    call_args: &[HirCallArgument],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    skip_self_param: bool,
) -> Option<OverloadCandidate> {
    let param_specs = if skip_self_param && candidate.param_specs.first().is_some_and(|param| param.name.name.as_str() == "self") {
        candidate.param_specs[1..].to_vec()
    }
    else {
        candidate.param_specs.clone()
    };
    let expected_types = param_specs.iter().map(|param| param.ty.clone()).collect::<Vec<_>>();
    let actual_types = if param_specs.is_empty() {
        call_args.iter().map(|arg| infer_scrutinee_type(&arg.value, locals, struct_fields, singleton_names)).collect::<Option<Vec<_>>>()?
    }
    else {
        let bound = bind_call_arguments(&param_specs, call_args).ok()?;
        bound.iter().map(|arg| infer_scrutinee_type(arg, locals, struct_fields, singleton_names)).collect::<Option<Vec<_>>>()?
    };
    let match_kind = compute_call_match_kind(type_relations, &actual_types, &expected_types)?;
    let mut matched = candidate.clone();
    matched.match_kind = match_kind;
    Some(matched)
}

pub(crate) fn try_resolve_singleton_method(
    owner: &Identifier,
    method_name: &Identifier,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    skip_self_param: bool,
    strip_receiver_arg: bool,
) -> Option<HirResolvedCall> {
    let call_args = if strip_receiver_arg && !args.is_empty() { &args[1..] } else { args };
    let filtered = candidates
        .iter()
        .filter(|candidate| candidate.owner.as_ref() == Some(owner))
        .filter(|candidate| matches!(candidate.domain, OverloadDomain::Function))
        .filter(|candidate| candidate_owned_method_matches(candidate, method_name))
        .filter_map(|candidate| {
            match_singleton_method_candidate(candidate, call_args, type_relations, locals, struct_fields, singleton_names, skip_self_param)
        })
        .collect::<Vec<_>>();
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    let has_receiver = candidate_has_receiver_parameter(&resolved);
    let symbol = overload_symbol_path(&resolved);
    let actual_receiver = if strip_receiver_arg && has_receiver {
        args.first().and_then(|arg| infer_scrutinee_type(&arg.value, locals, struct_fields, singleton_names))
    }
    else {
        None
    };
    let (return_type, parameter_types) = match (resolved.signature.params.first(), actual_receiver.as_ref()) {
        (Some(receiver_type), Some(actual_receiver)) => (
            substitute_type_parameters(&resolved.signature.return_type, receiver_type, actual_receiver),
            resolved.signature.params.iter().map(|param| substitute_type_parameters(param, receiver_type, actual_receiver)).collect(),
        ),
        _ => (resolved.signature.return_type, resolved.signature.params),
    };
    Some(HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol,
        domain: HirCallableDomain::Function,
        return_type,
        parameter_types,
        has_receiver,
        extractor_payload_type: None,
    })
}

pub(crate) fn try_resolve_type_static_method(
    type_owner: &Identifier,
    method_name: &Identifier,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> Option<HirResolvedCall> {
    let filtered = candidates
        .iter()
        .filter(|candidate| candidate.owner.as_ref() == Some(type_owner))
        .filter(|candidate| matches!(candidate.domain, OverloadDomain::Function | OverloadDomain::Constructor))
        .filter(|candidate| !candidate_has_receiver_parameter(candidate))
        .filter(|candidate| candidate_owned_method_matches(candidate, method_name))
        .filter_map(|candidate| {
            if let Some(context) = contextual_type
                && let Some(instantiated) = instantiate_candidate_for_context(candidate, context)
                && let Some(matched) = match_call_candidate(&instantiated, args, &[], type_relations, locals, struct_fields, singleton_names)
            {
                return Some(matched);
            }
            match_call_candidate(candidate, args, &[], type_relations, locals, struct_fields, singleton_names)
        })
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        return None;
    }
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    Some(HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol: overload_symbol_path(&resolved),
        domain: match resolved.domain {
            OverloadDomain::Constructor => HirCallableDomain::Constructor,
            _ => HirCallableDomain::Function,
        },
        return_type: substitute_self_type(&resolved.signature.return_type, Some(type_owner)),
        parameter_types: resolved.signature.params.iter().map(|ty| substitute_self_type(ty, Some(type_owner))).collect(),
        has_receiver: candidate_has_receiver_parameter(&resolved),
        extractor_payload_type: None,
    })
}

pub(crate) fn try_resolve_instance_method(
    receiver: &HirExpr,
    method_name: &Identifier,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<HirResolvedCall> {
    let receiver_arg = HirCallArgument::positional(receiver.clone());
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push(receiver_arg);
    full_args.extend_from_slice(args);

    let Some(receiver_type) =
        infer_expr_type(receiver, locals).or_else(|| infer_scrutinee_type(receiver, locals, struct_fields, singleton_names))
    else {
        return None;
    };
    // 内建数组形态 `[T]` 的运算符成员（`suffix [ ]` / `suffix ⁅ ⁆` 及其赋值形态）是
    // 语言原语：注册表候选只挂 `intrinsic`，返回裸元素 `T`。源码对 `Array<T>` 类写的
    // `imply` 运算符声明只服务类值，不得遮蔽内建数组形态的下标语义——源码里 `[T]⁅i⁆`
    // 一律按裸元素读（`Option<T>` 由 `get` 这类具名方法提供），否则同一份源码在元素
    // 具体时读内置候选、元素为形参时读声明候选，会得到两种返回类型。
    let prefer_array_intrinsic = matches!(receiver_type, ValkyrieType::Array(_) | ValkyrieType::FixedArray { .. })
        && operator_members::is_operator_member_name(method_name.as_str());
    let mut filtered = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| matches!(candidate.domain, OverloadDomain::Function))
        .filter(|candidate| {
            if prefer_array_intrinsic {
                candidate.declaration.is_none() && candidate.intrinsic.is_some()
            }
            else {
                candidate_has_receiver_parameter(candidate)
            }
        })
        .filter(|candidate| candidate_owned_method_matches(candidate, method_name) || candidate_trait_method_matches(candidate, method_name))
    {
        if let ValkyrieType::Generic(generic) = &receiver_type {
            if generic.binder.is_none() {
                continue;
            }
            let mut bounded = false;
            for bound in type_relations
                .callable_constraints()
                .iter()
                .filter(|constraint| Some(&constraint.target) == Some(&receiver_type))
                .flat_map(|constraint| &constraint.bounds)
                .filter(|bound| bound.declaration.is_some() && bound.declaration == candidate.trait_owner)
            {
                bounded = true;
                if let Some(candidate) = instantiate_trait_bound_candidate(candidate, &receiver_type, bound)
                    && let Some(matched) =
                        match_call_candidate(&candidate, &full_args, &[], type_relations, locals, struct_fields, singleton_names)
                {
                    filtered.push(matched);
                }
            }
            if !bounded {
                if let Some(candidate) = instantiate_trait_method_candidate(candidate, &receiver_type)
                    && let Some(matched) =
                        match_call_candidate(&candidate, &full_args, &[], type_relations, locals, struct_fields, singleton_names)
                {
                    filtered.push(matched);
                }
            }
        }
        else if candidate.trait_owner.is_none() {
            if let Some(matched) = match_call_candidate(candidate, &full_args, &[], type_relations, locals, struct_fields, singleton_names) {
                filtered.push(matched);
            }
        }
    }
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    let has_receiver = candidate_has_receiver_parameter(&resolved);
    let mut call = HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol: overload_symbol_path(&resolved),
        domain: HirCallableDomain::Function,
        return_type: resolved.signature.return_type,
        parameter_types: resolved.signature.params,
        has_receiver,
        extractor_payload_type: None,
    };
    enrich_sum_variant_operation_facts(&mut call, Some(&receiver_type), candidates);
    Some(call)
}
