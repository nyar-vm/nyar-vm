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
    patterns::sum_declaration_of_type,
};

pub(crate) fn paired_sum_identity_facts(sum_owner: Option<nyar_types::ItemId>, sum_variant: Option<nyar_types::ItemId>) -> bool {
    sum_owner.is_none() == sum_variant.is_none()
}
pub(crate) fn sum_apply_context(ty: &ValkyrieType) -> Option<ValkyrieType> {
    if is_option_apply_type(ty) || is_result_apply_type(ty) { Some(ty.clone()) } else { None }
}

pub(crate) fn option_apply_hint(
    contextual_type: Option<&ValkyrieType>,
    call_return_type: &ValkyrieType,
    explicit_generic_arguments: &[ValkyrieType],
) -> Option<ValkyrieType> {
    // 只消费已冻结的 Nominal unity 上下文；禁止用实参拼出 Named("Option")。
    let _ = explicit_generic_arguments;
    contextual_type.and_then(sum_apply_context).or_else(|| sum_apply_context(call_return_type))
}

pub(crate) fn enrich_unite_variant_constructor_facts(
    callee: &HirExpr,
    args: &[HirCallArgument],
    call: &mut HirResolvedCall,
    contextual_type: Option<&ValkyrieType>,
    candidates: &[OverloadCandidate],
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) {
    // domain 现已在 Constructor 专属路径保障，不会有 Function/intrinsic 候选误入。
    if call.sum_owner.is_some() && call.sum_variant.is_some() {
        return;
    }
    let Some(variant_name) = extract_callable_name(callee).or_else(|| match call.symbol.parts() {
        [name] => Some(name.clone()),
        _ => None,
    })
    else {
        return;
    };
    let (explicit_generic_arguments, _) = peel_explicit_generic_arguments(callee);
    let hint = sum_apply_context(&call.return_type)
        .or_else(|| contextual_type.and_then(sum_apply_context))
        .or_else(|| option_apply_hint(contextual_type, &call.return_type, &explicit_generic_arguments))
        .or_else(|| {
            if args.len() != 1 {
                return None;
            }
            let _payload = infer_scrutinee_type(&args[0].value, locals, struct_fields, singleton_names)?;
            candidates
                .iter()
                .filter(|candidate| candidate_variant_constructor_matches(candidate, &variant_name))
                .find_map(|candidate| candidate.signature.return_type.clone().into())
        });
    let Some(declaration) = hint.as_ref().and_then(|ty| sum_declaration_of_type(ty))
    else {
        return;
    };
    let Some((owner, variant)) = variant_constructor_facts_from_declaration(candidates, declaration, variant_name.as_str())
    else {
        return;
    };
    call.sum_owner = Some(owner);
    call.sum_variant = Some(variant);
    call.domain = HirCallableDomain::Constructor;
    call.declaration = None;
    call.instance = None;
    call.symbol = NamePath::new(vec![variant_name]);
    if let Some(ctx) = hint {
        call.return_type = ctx;
    }
}
fn variant_constructor_facts_from_declaration(
    candidates: &[OverloadCandidate],
    sum_owner: nyar_types::ItemId,
    variant_name: &str,
) -> Option<(nyar_types::ItemId, nyar_types::ItemId)> {
    candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Constructor)
        .filter(|candidate| candidate.sum_owner == Some(sum_owner))
        .filter(|candidate| candidate_variant_constructor_matches(candidate, &Identifier::new(variant_name)))
        .find_map(|candidate| candidate.sum_owner.zip(candidate.sum_variant))
}
pub(crate) fn instantiate_candidate_for_context(candidate: &OverloadCandidate, contextual_type: &ValkyrieType) -> Option<OverloadCandidate> {
    // 防御：candidate.return_type 最外层是裸 Generic binder 时（如 intrinsic seed
    // `__array_get(self: Array<T>): T`），禁止用外层 contextual_type 反推泛型。
    // 否则 Some(Option<T>) context 会把裸 T 绑定成 Option<T>，导致函数 appear 返回
    // Option<T>，再被 Some 包成 Option<Option<T>>。普通函数/构造器的返回类型总有
    // 外层形状（Option<T> / ArrayList<T> / Self / Nominal），裸 Generic 只在 intrinsic seed。
    if matches!(&candidate.signature.return_type, ValkyrieType::Generic(_)) {
        return None;
    }
    let mut substitutions = BTreeMap::new();
    unify_call_type_binders(&candidate.signature.return_type, contextual_type, &candidate.generic_binders, &mut substitutions)?;
    let mut candidate = candidate.clone();
    candidate.signature.return_type = substitute_type_vars(&candidate.signature.return_type, &substitutions);
    candidate.signature.params = candidate.signature.params.iter().map(|param| substitute_type_vars(param, &substitutions)).collect();
    Some(candidate)
}

/// 解析构造调用并返回带声明身份的调用合同；外部测试入口。
#[doc(hidden)]
pub fn try_resolve_constructor(
    path: &NamePath,
    args: &[HirExpr],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> Option<HirResolvedCall> {
    let name = path.parts().last()?.clone();
    if singleton_names.contains(&name) {
        return None;
    }
    let bound_values = if args.iter().all(|arg| matches!(arg.kind, HirExprKind::FieldInit { .. })) {
        if let Some(fields) = struct_fields.fields_of(&ValkyrieType::Named(name.clone())) {
            bind_construct_fields(fields, args).ok()?
        }
        else {
            // Unite/enum variant arms (`Field { owner, name }` for `MsilInstructionOperand`)
            // are constructors, not struct TypeDefs — keep FieldInit values in source order.
            args.iter()
                .filter_map(|arg| match &arg.kind {
                    HirExprKind::FieldInit { value, .. } => Some(value.as_ref().clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        }
    }
    else {
        args.to_vec()
    };
    let arguments = HirCallArgument::from_exprs(bound_values);
    let filtered = candidates
        .iter()
        .filter(|candidate| aggregate_constructor_matches_name(candidate, path))
        .filter_map(|candidate| {
            if let Some(context) = contextual_type {
                if let Some(instantiated) = instantiate_candidate_for_context(candidate, context) {
                    if let Some(matched) =
                        match_call_candidate(&instantiated, &arguments, &[], type_relations, locals, struct_fields, singleton_names)
                    {
                        return Some(matched);
                    }
                }
            }
            match_call_candidate(candidate, &arguments, &[], type_relations, locals, struct_fields, singleton_names)
        })
        .collect::<Vec<_>>();
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    if !paired_sum_identity_facts(resolved.sum_owner, resolved.sum_variant) {
        return None;
    }
    Some(HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol: overload_symbol_path(&resolved),
        domain: HirCallableDomain::Constructor,
        return_type: resolved.signature.return_type,
        parameter_types: resolved.signature.params,
        has_receiver: false,
        extractor_payload_type: None,
    })
}
