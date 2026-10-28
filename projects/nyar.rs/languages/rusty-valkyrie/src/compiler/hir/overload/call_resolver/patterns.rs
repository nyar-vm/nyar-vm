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

use super::core::candidate_has_receiver_parameter;

pub(super) fn sum_declaration_of_type(ty: &ValkyrieType) -> Option<nyar_types::ItemId> {
    match ty {
        ValkyrieType::Nominal { declaration, .. } => Some(*declaration),
        ValkyrieType::Apply(base, _) => sum_declaration_of_type(base),
        _ => None,
    }
}

fn enriched_sum_owner_variant(
    candidates: &[OverloadCandidate],
    sum_owner: nyar_types::ItemId,
    variant_name: &str,
) -> Option<(nyar_types::ItemId, nyar_types::ItemId)> {
    candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Constructor)
        .filter(|candidate| candidate.sum_owner == Some(sum_owner))
        .filter(|candidate| matches!(candidate.symbol.parts(), [name] if name.as_str() == variant_name))
        .find_map(|candidate| candidate.sum_owner.zip(candidate.sum_variant))
}

pub(crate) fn enrich_sum_variant_operation_facts(
    call: &mut HirResolvedCall,
    receiver_type: Option<&ValkyrieType>,
    candidates: &[OverloadCandidate],
) {
    if call.sum_owner.is_some() && call.sum_variant.is_some() {
        return;
    }
    let Some(receiver_type) = receiver_type
    else {
        return;
    };
    let method_name = call.symbol.parts().last().map(|part| part.as_str());
    let variant_name = match method_name {
        Some("unwrap") | Some("is_some") if is_option_apply_type(receiver_type) => "Some",
        _ => return,
    };
    let Some(declaration) = sum_declaration_of_type(receiver_type)
    else {
        return;
    };
    let Some((owner, variant)) = enriched_sum_owner_variant(candidates, declaration, variant_name)
    else {
        return;
    };
    call.sum_owner = Some(owner);
    call.sum_variant = Some(variant);
    if method_name == Some("unwrap") {
        call.domain = HirCallableDomain::Extractor;
        if call.extractor_payload_type.is_none() {
            call.extractor_payload_type = option_apply_payload_type(receiver_type).or_else(|| nullable_payload_type(&call.return_type));
        }
    }
}

pub(crate) fn try_resolve_pattern_extractor(
    canonical_callee: &NamePath,
    actual_type: &ValkyrieType,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
) -> Option<HirResolvedCall> {
    fn matches_extractor_receiver_type(
        actual: &ValkyrieType,
        receiver: &ValkyrieType,
        type_relations: &TypeRelationContext,
    ) -> Option<OverloadMatchKind> {
        if let Some(kind) = compute_call_match_kind(type_relations, std::slice::from_ref(actual), std::slice::from_ref(receiver)) {
            return Some(kind);
        }
        None
    }
    let filtered = candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Function)
        .filter(|candidate| candidate.owner.as_ref().is_some_and(|owner| canonical_callee.parts().first() == Some(owner)))
        .filter(|candidate| candidate.param_specs.first().is_some_and(|p| p.name.name.as_str() == "self" && !p.is_mutable))
        .filter(|candidate| candidate_extractor_path_matches(canonical_callee, candidate))
        .filter_map(|candidate| {
            let receiver_type = pattern_extractor_receiver_type(candidate)?;
            let match_kind = matches_extractor_receiver_type(actual_type, &receiver_type, type_relations)?;
            let mut matched = candidate.clone();
            matched.domain = OverloadDomain::Extractor;
            matched.match_kind = match_kind;
            Some(matched)
        })
        .collect::<Vec<_>>();
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    let has_receiver = candidate_has_receiver_parameter(&resolved);
    let receiver_type = pattern_extractor_receiver_type(&resolved)?;
    let return_type = substitute_type_parameters(&resolved.signature.return_type, &receiver_type, actual_type);
    let payload_type = nullable_payload_type(&return_type)?;
    let parameter_types =
        resolved.signature.params.iter().map(|param| substitute_type_parameters(param, &receiver_type, actual_type)).collect();
    Some(HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        trait_dispatch: None,
        symbol: resolved.symbol,
        domain: HirCallableDomain::Extractor,
        return_type,
        parameter_types,
        has_receiver,
        extractor_payload_type: Some(payload_type),
    })
}

pub(crate) fn try_resolve_try_propagate_ok_variant(operand_type: &ValkyrieType, candidates: &[OverloadCandidate]) -> Option<HirResolvedCall> {
    let declaration = sum_declaration_of_type(operand_type)?;
    let (variant_name, payload_type) = if is_result_apply_type(operand_type) {
        ("Fine", result_variant_payload_type(operand_type, "Fine"))
    }
    else if is_option_apply_type(operand_type) {
        ("Some", option_apply_payload_type(operand_type))
    }
    else {
        return None;
    };
    let Some((owner, variant)) = enriched_sum_owner_variant(candidates, declaration, variant_name)
    else {
        return None;
    };
    Some(HirResolvedCall {
        declaration: None,
        instance: None,
        sum_owner: Some(owner),
        sum_variant: Some(variant),
        intrinsic: None,
        trait_dispatch: None,
        symbol: NamePath::new(vec![Identifier::new(variant_name)]),
        domain: HirCallableDomain::Extractor,
        return_type: payload_type.clone().unwrap_or(ValkyrieType::Unit),
        parameter_types: vec![operand_type.clone()],
        has_receiver: false,
        extractor_payload_type: payload_type,
    })
}
