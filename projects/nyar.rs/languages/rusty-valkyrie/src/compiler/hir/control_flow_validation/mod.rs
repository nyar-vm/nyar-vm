use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::{
        is_nullable_type, is_option_apply_type, is_result_apply_type,
        lowering::{
            HirError,
            errors::{ControlFlowOp, LoopJumpOperation, PropagateProtocol, RequiredContext},
        },
        nullable_payload_type,
    },
    semantic::control_flow::{ControlFlowContext, TryScopeData},
    types::{
        Identifier, NamePath,
        hir::{
            HirBlock, HirExpr, HirExprKind, HirFunction, HirLiteral, HirMatchArm, HirModule, HirPattern, HirStatement, HirStatementKind,
            ValkyrieType,
        },
    },
};

use nyar_types::{DiagnosticSource, DiagnosticSpan};

mod compatibility;
mod module_walk;
mod pattern;
mod state;
mod suspend;
mod validate;

pub(crate) use state::{HirValidationState, boolean_declarations_in_module, numeric_declarations_in_module};

pub use compatibility::control_flow_value_type_compatible;
pub(crate) use compatibility::display_type;
pub use module_walk::validate_control_flow_module;
pub use suspend::{
    function_body_contains_await, function_body_contains_block_on, function_body_contains_yield, function_can_effect_handle,
    function_can_suspend, function_has_suspend_effects, function_is_async, function_may_yield_or_await, function_needs_suspend_fragment,
};

pub(super) fn control_flow_source() -> nyar_types::DiagnosticSource {
    nyar_types::DiagnosticSource { id: "<control_flow>".into(), revision: None }
}

pub(super) fn expr_diagnostic_span(expr: &crate::compiler::types::hir::HirExpr) -> nyar_types::DiagnosticSpan {
    nyar_types::DiagnosticSpan { start: expr.span.span.start as usize, end: expr.span.span.end as usize }
}
