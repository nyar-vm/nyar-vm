//! Pattern / extractor policy helpers for `resolve_call`.

use crate::compiler::{
    hir::{
        overload::{
            diagnostics::{OverloadCandidate, OverloadDomain},
            symbol_match::candidate_extractor_path_matches,
            types::compute_call_match_kind,
        },
        try_propagate::nullable_payload_type,
        type_relation::{ParameterMatchResult, TypeRelationContext},
    },
    syntax::oak::ParseError,
    types::{
        NamePath,
        hir::{HirExtractorPattern, HirParam, HirPattern, ValkyrieType},
    },
};

use super::validate::{is_builtin_pattern_extractor, is_option_owner_type, is_result_owner_type};

/// `Some` / `None` on `T | null` (and `Option<T>`) are built-in arms — not user extractors.
pub(super) fn is_builtin_nullable_option_pattern(canonical_callee: &NamePath, actual_type: &ValkyrieType) -> bool {
    let head = canonical_callee.parts().first().map(|p| p.as_str()).unwrap_or("");
    if !matches!(head, "Some" | "None") {
        return false;
    }
    crate::compiler::hir::is_nullable_type(actual_type) || is_option_owner_type(actual_type)
}

/// `Fine` / `Fail` are the language Result-unite arms. Concrete aliases
/// (`VonParseResult`, `JvmBinaryResult`, …) still use those variant names, so
/// accept Fine/Fail extractors for any Result-shaped apply (`Result<…>` or
/// `*Result<…>`) rather than only the nominal `Result` constructor.
pub(super) fn is_builtin_result_pattern(canonical_callee: &NamePath, actual_type: &ValkyrieType) -> bool {
    let head = canonical_callee.parts().first().map(|p| p.as_str()).unwrap_or("");
    if !matches!(head, "Fine" | "Fail") {
        return false;
    }
    if crate::compiler::hir::is_result_apply_type(actual_type) || is_result_owner_type(actual_type) {
        return true;
    }
    is_result_owner_type(actual_type)
}

/// `Text` / `Object` / … on `VonValue` are language VON unite arms (legion.tools manifest.v).
pub(super) fn is_builtin_von_value_pattern(canonical_callee: &NamePath, actual_type: &ValkyrieType) -> bool {
    let head = canonical_callee.parts().first().map(|p| p.as_str()).unwrap_or("");
    if !matches!(head, "Text" | "Flag" | "Number" | "Name" | "Array" | "Object" | "Empty") {
        return false;
    }
    matches!(actual_type, ValkyrieType::Named(name) if name.as_str() == "VonValue")
}

/// 当 pattern 名（`canonical_callee` 首段）是 scrutinee 类型的严格子类时，
/// 该 pattern 为类型测试 pattern，不需要 extractor 方法。
/// 同名类型（NominalExact）不算类型测试，仍需 extractor。
pub(super) fn is_subtype_pattern(canonical_callee: &NamePath, actual_type: &ValkyrieType, type_relations: &TypeRelationContext) -> bool {
    let Some(pattern_name) = canonical_callee.parts().first()
    else {
        return false;
    };
    let scrutinee_name = match actual_type {
        ValkyrieType::Named(name) => name.clone(),
        ValkyrieType::Apply(base, _) => {
            if let ValkyrieType::Named(name) = base.as_ref() {
                name.clone()
            }
            else {
                return false;
            }
        }
        _ => return false,
    };
    matches!(type_relations.match_nominal_parameter(pattern_name, &scrutinee_name), ParameterMatchResult::NominalSubtype { .. })
}

pub(super) fn diagnose_pattern_extractor_failure(
    canonical_callee: &NamePath,
    actual_type: &ValkyrieType,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
) -> Option<ParseError> {
    if is_builtin_pattern_extractor(canonical_callee) {
        return None;
    }

    let type_name = render_valkyrie_type_name(actual_type);
    let extractor_name = canonical_callee.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>().join(".");

    let matching = matching_extractor_candidates(canonical_callee, actual_type, candidates, type_relations);
    if matching.is_empty() {
        if is_subtype_pattern(canonical_callee, actual_type, type_relations) {
            return None;
        }
        if is_builtin_nullable_option_pattern(canonical_callee, actual_type) {
            return None;
        }
        if is_builtin_result_pattern(canonical_callee, actual_type) {
            return None;
        }
        if is_builtin_von_value_pattern(canonical_callee, actual_type) {
            return None;
        }
        return Some(ParseError::Invalid {
            message: format!("unknown pattern extractor `{extractor_name}` for scrutinee type `{type_name}`"),
            span: None,
        });
    }

    if matching
        .iter()
        .any(|candidate| candidate.param_specs.first().is_some_and(|param| is_extractor_receiver_param(param) && param.is_mutable))
    {
        return Some(ParseError::Invalid {
            message: format!("pattern extractor `{extractor_name}` must not use `mut self` on `{type_name}`"),
            span: None,
        });
    }

    if matching.iter().all(|candidate| nullable_payload_type(&candidate.signature.return_type).is_none()) {
        let return_type = matching
            .first()
            .map(|candidate| render_valkyrie_type_name(&candidate.signature.return_type))
            .unwrap_or_else(|| "unknown".to_string());
        return Some(ParseError::Invalid {
            message: format!("pattern extractor `{extractor_name}` must return a nullable type (`T?`); got `{return_type}` for `{type_name}`"),
            span: None,
        });
    }

    None
}

pub(super) fn diagnose_unresolved_extractor_without_scrutinee_type(
    canonical_callee: &NamePath,
    candidates: &[OverloadCandidate],
) -> Option<ParseError> {
    let head = canonical_callee.parts().first().map(|p| p.as_str()).unwrap_or("");
    if matches!(head, "Fine" | "Fail" | "Some" | "None") {
        return Some(ParseError::Invalid {
            message: format!("pattern extractor `{head}` 需要已解析的 scrutinee 类型，不能仅凭变体名放行"),
            span: None,
        });
    }
    if matches!(head, "Text" | "Flag" | "Number" | "Name" | "Array" | "Object" | "Empty") {
        return None;
    }
    if matches!(canonical_callee.parts(), [owner, member] if owner.as_str() == head && member.as_str() == "extractor")
        && candidates
            .iter()
            .any(|candidate| candidate.domain == OverloadDomain::Function && candidate_extractor_path_matches(canonical_callee, candidate))
    {
        return None;
    }

    let extractor_name = canonical_callee.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>().join(".");
    Some(ParseError::Invalid {
        message: format!("cannot validate pattern extractor `{extractor_name}` without a known scrutinee type"),
        span: None,
    })
}

pub(super) fn diagnose_unresolved_arm_extractor(pattern: &HirPattern, candidates: &[OverloadCandidate]) -> Option<ParseError> {
    match pattern {
        HirPattern::Extractor(HirExtractorPattern::Constructor { canonical_callee, resolved, .. })
        | HirPattern::Extractor(HirExtractorPattern::Array { canonical_callee, resolved, .. })
            if resolved.is_none() =>
        {
            diagnose_unresolved_extractor_without_scrutinee_type(canonical_callee, candidates)
        }
        HirPattern::Or(items) => items.iter().find_map(|item| diagnose_unresolved_arm_extractor(item, candidates)),
        HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
            diagnose_unresolved_arm_extractor(pattern, candidates)
        }
        _ => None,
    }
}

pub(super) fn matching_extractor_candidates<'a>(
    canonical_callee: &NamePath,
    actual_type: &ValkyrieType,
    candidates: &'a [OverloadCandidate],
    type_relations: &TypeRelationContext,
) -> Vec<&'a OverloadCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Function)
        .filter(|candidate| candidate_extractor_path_matches(canonical_callee, candidate))
        .filter(|candidate| candidate.param_specs.first().is_some_and(is_extractor_receiver_param))
        .filter(|candidate| {
            pattern_extractor_receiver_type(candidate)
                .and_then(|receiver_type| matches_extractor_receiver_type(actual_type, &receiver_type, type_relations))
                .is_some()
        })
        .collect()
}

fn is_extractor_receiver_param(param: &HirParam) -> bool {
    param.name.name.as_str() == "self"
        && matches!(
            param.ty,
            ValkyrieType::r#SelfType
                | ValkyrieType::AutoType
                | ValkyrieType::Named(_)
                | ValkyrieType::Nominal { .. }
                | ValkyrieType::Apply(_, _)
        )
}

pub(crate) fn pattern_extractor_receiver_type(candidate: &OverloadCandidate) -> Option<ValkyrieType> {
    if let Some(param) = candidate.param_specs.first().filter(|param| is_extractor_receiver_param(param)) {
        if matches!(param.ty, ValkyrieType::r#SelfType | ValkyrieType::AutoType) {
            if let Some(owner) = &candidate.owner {
                return Some(ValkyrieType::Named(owner.clone()));
            }
        }
        return Some(param.ty.clone());
    }
    if matches!(candidate.signature.params.first(), Some(ValkyrieType::r#SelfType)) {
        return candidate
            .owner
            .as_ref()
            .map(|owner| ValkyrieType::Named(owner.clone()))
            .or_else(|| candidate.signature.params.first().cloned());
    }
    candidate.owner.as_ref().map(|owner| ValkyrieType::Named(owner.clone()))
}

fn matches_extractor_receiver_type(
    actual: &ValkyrieType,
    receiver: &ValkyrieType,
    type_relations: &TypeRelationContext,
) -> Option<crate::compiler::hir::overload::diagnostics::OverloadMatchKind> {
    compute_call_match_kind(type_relations, std::slice::from_ref(actual), std::slice::from_ref(receiver))
}

fn render_valkyrie_type_name(ty: &ValkyrieType) -> String {
    match ty {
        ValkyrieType::Named(name) => name.as_str().to_string(),
        ValkyrieType::Tuple(items) => {
            let inner = items.iter().map(render_valkyrie_type_name).collect::<Vec<_>>().join(", ");
            format!("({inner})")
        }
        ValkyrieType::Array(inner) => format!("Array<{}>", render_valkyrie_type_name(inner)),
        ValkyrieType::Union(items) => items.iter().map(render_valkyrie_type_name).collect::<Vec<_>>().join(" | "),
        ValkyrieType::Apply(base, args) => {
            let rendered_args = args.iter().map(render_valkyrie_type_name).collect::<Vec<_>>().join(", ");
            format!("{}<{}>", render_valkyrie_type_name(base), rendered_args)
        }
        ValkyrieType::r#SelfType => "Self".to_string(),
        other => format!("{other:?}"),
    }
}
