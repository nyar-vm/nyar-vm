use crate::compiler::types::hir::{HirExpr, HirExprKind, HirExtractorPattern, HirResolvedCall, ValkyrieType};

pub(super) fn extractor_resolved_call(extractor: &HirExtractorPattern) -> Option<&HirResolvedCall> {
    match extractor {
        HirExtractorPattern::Constructor { resolved, .. } | HirExtractorPattern::Array { resolved, .. } => resolved.as_ref(),
    }
}

pub(super) fn tuple_literal_exprs(expr: &HirExpr) -> Option<Vec<HirExpr>> {
    match &expr.kind {
        HirExprKind::TupleLiteral { items } => Some(items.clone()),
        _ => None,
    }
}

/// Match the already linked nominal owner.  Source spelling aliases must be
/// resolved by HIR/linker identity before pattern lowering; this function must
/// not infer a sum declaration from a name suffix.
pub(super) fn sum_type_name_matches(a: &str, b: &str) -> bool {
    a == b
}

/// HIR extractor metadata can retain a generic parameter when no concrete substitution is
/// available. It must not win over an already-concrete SSA type.
pub(super) fn looks_like_unbound_type_parameter(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::Generic(_))
}
