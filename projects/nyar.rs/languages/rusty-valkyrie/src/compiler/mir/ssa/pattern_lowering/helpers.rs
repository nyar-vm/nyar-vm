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

/// HIR extractor 元数据可能保留泛型形参。只有缺失 `binder` 的形参才是未解析占位；
/// 带 `binder` 的形参是前端冻结的声明身份（与 canonical `Generic{owner, ordinal}` 同口径）。
pub(super) fn looks_like_unbound_type_parameter(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::Generic(generic) if generic.binder.is_none())
}

/// HIR 携带的参数化类型提示（无论是否已有 `binder`）不得压过 SSA 上更具体的类型事实。
pub(super) fn is_generic_parameter(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::Generic(_))
}
