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

use super::{control_flow_source, expr_diagnostic_span};

use crate::compiler::hir::control_flow_validation::{
    pattern::{pop_self_type, push_self_type},
    state::{numeric_declarations_in_module, validation_state},
    validate::validate_hir_function,
};

/// 校验模块内全部函数与控制流的 HIR 合法性。
pub fn validate_control_flow_module(module: &HirModule) -> Result<(), HirError> {
    let numeric_declarations = numeric_declarations_in_module(module);
    for function in &module.functions {
        let mut state = validation_state(&numeric_declarations);
        validate_hir_function(function, &mut state)?;
    }
    for struct_def in &module.structs {
        for method in &struct_def.methods {
            let mut state = validation_state(&numeric_declarations);
            push_self_type(&mut state, ValkyrieType::Named(struct_def.name.clone()));
            validate_hir_function(method, &mut state)?;
            pop_self_type(&mut state);
        }
        for property in &struct_def.properties {
            if let Some(getter) = &property.getter {
                let mut state = validation_state(&numeric_declarations);
                push_self_type(&mut state, ValkyrieType::Named(struct_def.name.clone()));
                validate_hir_function(getter, &mut state)?;
                pop_self_type(&mut state);
            }
            if let Some(setter) = &property.setter {
                let mut state = validation_state(&numeric_declarations);
                push_self_type(&mut state, ValkyrieType::Named(struct_def.name.clone()));
                validate_hir_function(setter, &mut state)?;
                pop_self_type(&mut state);
            }
        }
    }
    for trait_def in &module.traits {
        for method in &trait_def.methods {
            let mut state = validation_state(&numeric_declarations);
            validate_hir_function(method, &mut state)?;
        }
        for method in &trait_def.default_methods {
            let mut state = validation_state(&numeric_declarations);
            validate_hir_function(method, &mut state)?;
        }
    }
    for impl_block in &module.impls {
        for method in &impl_block.methods {
            let mut state = validation_state(&numeric_declarations);
            push_self_type(&mut state, impl_block.target.clone());
            validate_hir_function(method, &mut state)?;
            pop_self_type(&mut state);
        }
    }
    Ok(())
}
