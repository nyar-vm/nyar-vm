use crate::compiler::{
    hir::is_nullable_type,
    types::{
        Identifier, NamePath,
        hir::{FunctionType, HirCallableDomain, HirExpr, HirExprKind, HirLiteral, HirResolvedCall, ValkyrieType},
    },
};
use nyar_types::IntrinsicId;

use super::{
    MirBuilder, MirConstant, MirInstruction, MirOperand, MirOperation, MirStorageKind, MirValueOrigin, callee_name_matches,
    expr_helpers::{is_array_shaped_valkyrie_type, lower_resolved_callee, named_type_name, peel_generic_apply},
    infer_builder_operand_type, lower_callee_operand,
    value_semantics::{ensure_layout_for_type, layout_id_for_type, storage_kind_for_type},
};

mod intrinsics;
mod literals;
mod operand;

pub(crate) use literals::{
    apply_payload_to_sum_type_args, concretize_variant_constructor_types, instantiated_payload_for_call, lower_literal, refine_auto_apply_type,
    refine_auto_sum_instantiation_args, sum_new_parts_from_constructor, type_args_from_sum_shaped, unresolved_variant_labels,
};
