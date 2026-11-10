mod helpers;

use std::collections::BTreeSet;

use crate::compiler::types::{
    Identifier, NamePath,
    hir::{HirExpr, HirExprKind, HirExtractorPattern, HirLiteral, HirPattern, HirResolvedCall, ValkyrieType},
};

use crate::compiler::hir::nullable_payload_type;

use crate::compiler::mir::ssa::{
    MirBuilder, MirConstant, MirDiagnostic, MirInstruction, MirOperand, MirOperation, MirTerminator, MirValueOrigin,
    MirValueRef,
    expr_lowering::lower_literal,
    infer_builder_operand_type, lower_resolved_callee, named_type_name,
    value_semantics::{LayoutId, ensure_unite_tagged_layout, layout_id_for_type, value_copy_supported},
};

use helpers::{extractor_resolved_call, is_generic_parameter, looks_like_unbound_type_parameter, sum_type_name_matches, tuple_literal_exprs};

/// sum 构造 pattern 的载荷计划：probe 阶段只登记事实、不发射指令；
/// 由匹配成功后继块顶部 `materialize_probe_payload` 落成 `SumPayloadGet`。
pub(crate) struct DeferredSumPayload {
    object: MirOperand,
    nominal: nyar_types::NominalInstanceId,
    type_args: Vec<ValkyrieType>,
    variant: nyar_types::VariantId,
    payload_type: ValkyrieType,
}

/// `lower_pattern_match_probe` 返回的载荷：已物化操作数，或待物化的 sum 载荷计划。
pub(crate) enum ProbePayload {
    /// 已在 probe 块物化（自定义 extractor 调用结果等非 sum 路径）。
    Materialized(MirOperand),
    /// sum 变体载荷：须在匹配成功后继块物化，避免在未匹配路径读取 null 载荷。
    DeferredSum(DeferredSumPayload),
}

include!("bind.rs");
include!("match_operand.rs");
