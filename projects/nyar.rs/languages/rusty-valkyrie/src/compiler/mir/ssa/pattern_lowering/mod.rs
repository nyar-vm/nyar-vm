mod helpers;

use std::collections::BTreeSet;

use crate::compiler::types::{
    Identifier, NamePath,
    hir::{HirExpr, HirExprKind, HirExtractorPattern, HirLiteral, HirPattern, HirResolvedCall, ValkyrieType},
};

use crate::compiler::hir::nullable_payload_type;

use crate::compiler::mir::ssa::{
    MirBuilder, MirConstant, MirDiagnostic, MirInstruction, MirOperand, MirOperation, MirStorageKind, MirTerminator, MirValueOrigin,
    MirValueRef,
    expr_lowering::lower_literal,
    infer_builder_operand_type, lower_resolved_callee, named_type_name,
    value_semantics::{LayoutId, ensure_layout_for_type, ensure_unite_tagged_layout, layout_id_for_type, storage_kind_for_type},
};

use helpers::{extractor_resolved_call, is_generic_parameter, looks_like_unbound_type_parameter, sum_type_name_matches, tuple_literal_exprs};

include!("bind.rs");
include!("match_operand.rs");
