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
    compatibility::{
        control_flow_value_type_compatible, control_flow_value_type_compatible_with_primitives, display_type, future_resume_type,
        generator_yield_type, integer_literal_binds_to, return_without_value_compatible,
    },
    pattern::{
        bind_local_type, bind_pattern_type, collect_pattern_bound_names, current_return_type, current_self_type,
        detect_case_fallthrough_binding_leak, expr_contains_fallthrough, lookup_local_type, pop_local_scope, pop_return_type, push_local_scope,
        push_return_type, validate_pattern_semantics,
    },
    state::{HirValidationState, LoopValidationContext, validation_state},
    suspend::{block_contains_yield, function_body_contains_await, function_body_contains_yield, function_is_async},
};
pub fn validate_hir_function(function: &HirFunction, state: &mut HirValidationState) -> Result<(), HirError> {
    let saved_function = state.current_function.clone();
    state.current_function = Some(function.name.to_string());
    let is_async = function_is_async(&function.body);
    let allow_blocking_value = !is_async;
    let is_generator = function_body_contains_yield(&function.body);
    state.control_flow.push_async(None, is_async);
    if let Some(scope) = state.control_flow.current_async_scope_mut() {
        scope.allow_blocking = allow_blocking_value;
    }
    if is_generator {
        state.control_flow.push_generator(None, is_async);
        if let Some(scope) = state.control_flow.current_generator_scope_mut() {
            scope.allow_yield = true;
        }
    }
    push_return_type(state, resolve_contextual_type(&function.return_type, state));
    push_local_scope(state);
    for param in &function.params {
        bind_local_type(state, param.name.name.as_str(), resolve_contextual_type(&param.ty, state));
    }
    for statement in &function.body.statements {
        validate_hir_statement(statement, state)?;
    }
    if let Some(expr) = &function.body.expr {
        validate_hir_expr(expr, state, true)?;
    }
    pop_local_scope(state);
    pop_return_type(state);
    if is_generator {
        state.control_flow.pop_generator();
    }
    state.control_flow.pop_async();
    state.current_function = saved_function;
    Ok(())
}

pub fn validate_hir_statement(statement: &HirStatement, state: &mut HirValidationState) -> Result<(), HirError> {
    match &statement.kind {
        HirStatementKind::Let { pattern, initializer, ty, .. } => {
            if let Some(initializer) = initializer {
                validate_hir_expr(initializer, state, true)?;
            }
            validate_pattern_semantics(pattern)?;
            let inferred_type = ty.clone().or_else(|| initializer.as_ref().and_then(|value| infer_static_expr_type(value, state)));
            if let Some(inferred_type) = inferred_type {
                bind_pattern_type(state, pattern, &inferred_type);
            }
        }
        HirStatementKind::Expr(expr) => validate_hir_expr(expr, state, false)?,
    }
    Ok(())
}

pub fn validate_hir_block(block: &HirBlock, state: &mut HirValidationState, value_context: bool) -> Result<(), HirError> {
    push_local_scope(state);
    for statement in &block.statements {
        validate_hir_statement(statement, state)?;
    }
    if let Some(expr) = &block.expr {
        validate_hir_expr(expr, state, value_context)?;
    }
    pop_local_scope(state);
    Ok(())
}

pub fn validate_hir_expr(expr: &HirExpr, state: &mut HirValidationState, value_context: bool) -> Result<(), HirError> {
    match &expr.kind {
        HirExprKind::Call { callee, args, .. } => {
            validate_hir_expr(callee, state, true)?;
            for arg in args {
                validate_hir_expr(&arg.value, state, true)?;
            }
        }
        HirExprKind::FieldInit { value, .. }
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::TryPropagate { inner: value, .. }
        | HirExprKind::Raise(value)
        | HirExprKind::Resume(value) => {
            if matches!(expr.kind, HirExprKind::Resume(_))
                && (state.control_flow.current_catch_depth() == 0 || !state.control_flow.current_catch_arm_body())
            {
                return Err(HirError::ControlFlowContextViolation {
                    operation: ControlFlowOp::Resume,
                    required: RequiredContext::CatchArmBody,
                    source: control_flow_source(),
                    span: expr_diagnostic_span(expr),
                });
            }
            if state.in_guard {
                let guard_operation = match &expr.kind {
                    HirExprKind::Await(_) => Some(ControlFlowOp::Await),
                    HirExprKind::Awake(_) => Some(ControlFlowOp::Awake),
                    HirExprKind::BlockOn(_) => Some(ControlFlowOp::Block),
                    HirExprKind::YieldFrom(_) => Some(ControlFlowOp::YieldFrom),
                    HirExprKind::Resume(_) => Some(ControlFlowOp::Resume),
                    HirExprKind::TryPropagate { .. } => Some(ControlFlowOp::TryPropagate),
                    _ => None,
                };
                if let Some(operation) = guard_operation {
                    return Err(HirError::ControlFlowContextViolation {
                        operation,
                        required: RequiredContext::OutsideGuard,
                        source: control_flow_source(),
                        span: expr_diagnostic_span(expr),
                    });
                }
            }
            if matches!(expr.kind, HirExprKind::BlockOn(_)) {
                validate_blocking_context(expr, state)?;
            }
            if matches!(expr.kind, HirExprKind::Await(_)) {
                validate_await_context(expr, state)?;
            }
            if matches!(expr.kind, HirExprKind::YieldFrom(_)) {
                validate_yield_context(expr, state)?;
            }
            if matches!(expr.kind, HirExprKind::TryPropagate { .. }) {
                validate_try_propagate_context(expr, state)?;
            }
            validate_hir_expr(value, state, true)?;
            if matches!(expr.kind, HirExprKind::YieldFrom(_)) {
                validate_yield_from_value(expr, value, state)?;
            }
            if matches!(expr.kind, HirExprKind::Await(_) | HirExprKind::Awake(_) | HirExprKind::BlockOn(_)) {
                validate_future_control_operand(expr, value, state)?;
            }
        }
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            for arg in args {
                validate_hir_expr(arg, state, true)?;
            }
        }
        HirExprKind::Fallthrough => {
            let in_case_arm = state.control_flow.current_case_chain_arm_body();
            let in_catch_arm = state.control_flow.current_catch_arm_body();
            let has_fallthrough_target = state.control_flow.current_case_chain_scope().is_some_and(|scope| scope.next_arm_target.is_some());
            if in_case_arm && has_fallthrough_target && !in_catch_arm {
                return Ok(());
            }
            if in_case_arm && !has_fallthrough_target {
                return Err(HirError::FallthroughWithoutNextArm { source: control_flow_source(), span: expr_diagnostic_span(expr) });
            }
            return Err(HirError::FallthroughOutsideCaseChain { source: control_flow_source(), span: expr_diagnostic_span(expr) });
        }
        HirExprKind::With { base, updates } => {
            validate_hir_expr(base, state, true)?;
            for (_, value) in updates {
                validate_hir_expr(value, state, true)?;
            }
        }
        HirExprKind::TryScope { is_optional, is_forced, result_type, body } => {
            state.control_flow.push_try(TryScopeData { is_optional: *is_optional, is_forced: *is_forced, result_type: result_type.clone() });
            validate_hir_block(body, state, true)?;
            state.control_flow.pop_try();
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                validate_hir_expr(arg, state, true)?;
            }
        }
        HirExprKind::ArrayNew { length, .. } => validate_hir_expr(length, state, true)?,
        HirExprKind::FieldAccess { object, .. } => validate_hir_expr(object, state, true)?,
        HirExprKind::StoreField { object, value, .. } => {
            validate_hir_expr(object, state, true)?;
            validate_hir_expr(value, state, true)?;
        }
        HirExprKind::GenericApply { callee, .. } => validate_hir_expr(callee, state, true)?,
        HirExprKind::Block(block) => validate_hir_block(block, state, value_context)?,
        HirExprKind::Lambda { params, body, .. } => {
            let mut lambda_state = lambda_validation_state(state, body);
            if let HirExprKind::Lambda { return_type, .. } = &expr.kind {
                push_return_type(&mut lambda_state, return_type.clone());
            }
            let lambda_is_async = function_body_contains_await(body);
            let lambda_contains_yield = block_contains_yield(body);
            lambda_state.control_flow.push_async(None, lambda_is_async);
            if lambda_contains_yield {
                lambda_state.control_flow.push_generator(None, lambda_is_async);
                if let Some(scope) = lambda_state.control_flow.current_generator_scope_mut() {
                    scope.allow_yield = true;
                }
            }
            push_local_scope(&mut lambda_state);
            for param in params {
                bind_local_type(&mut lambda_state, param.name.name.as_str(), param.ty.clone());
            }
            validate_hir_block(body, &mut lambda_state, true)?;
            pop_local_scope(&mut lambda_state);
            if lambda_contains_yield {
                lambda_state.control_flow.pop_generator();
            }
            lambda_state.control_flow.pop_async();
            pop_return_type(&mut lambda_state);
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                validate_hir_expr(value, state, true)?;
            }
            for method in methods {
                validate_hir_function(method, &mut validation_state(&state.numeric_declarations, &state.boolean_declarations))?;
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            validate_hir_expr(condition, state, true)?;
            validate_hir_block(then_branch, state, value_context)?;
            if let Some(else_branch) = else_branch {
                validate_hir_block(else_branch, state, value_context)?;
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            validate_hir_expr(scrutinee, state, true)?;
            push_local_scope(state);
            validate_pattern_semantics(pattern)?;
            if let Some(scrutinee_type) = infer_static_expr_type(scrutinee, state) {
                bind_pattern_type(state, pattern, &scrutinee_type);
            }
            validate_hir_block(then_branch, state, value_context)?;
            pop_local_scope(state);
            if let Some(else_branch) = else_branch {
                validate_hir_block(else_branch, state, value_context)?;
            }
        }
        HirExprKind::Match { scrutinee, arms } => {
            validate_hir_expr(scrutinee, state, true)?;
            let scrutinee_type = infer_static_expr_type(scrutinee, state);
            for arm in arms {
                push_local_scope(state);
                validate_pattern_semantics(&arm.pattern)?;
                if let Some(scrutinee_type) = &scrutinee_type {
                    bind_pattern_type(state, &arm.pattern, scrutinee_type);
                }
                let saved_in_guard = state.in_guard;
                state.in_guard = true;
                if let Some(guard) = &arm.guard {
                    validate_hir_expr(guard, state, true)?;
                }
                state.in_guard = saved_in_guard;
                validate_hir_expr(&arm.body, state, value_context)?;
                pop_local_scope(state);
            }
        }
        HirExprKind::Case { scrutinee, arms } => {
            validate_hir_expr(scrutinee, state, true)?;
            let scrutinee_type = infer_static_expr_type(scrutinee, state);
            let mut previous_fallthrough_bindings = BTreeSet::new();
            state.control_flow.push_case_chain(None, None, arms.len() > 1);
            for (index, arm) in arms.iter().enumerate() {
                if let Some(leaked_name) = detect_case_fallthrough_binding_leak(arm, &previous_fallthrough_bindings, state) {
                    state.control_flow.pop_case_chain();
                    return Err(HirError::FallthroughBindingLeak {
                        name: leaked_name,
                        source: control_flow_source(),
                        span: expr_diagnostic_span(&arm.body),
                    });
                }
                push_local_scope(state);
                validate_pattern_semantics(&arm.pattern)?;
                if let Some(scrutinee_type) = &scrutinee_type {
                    bind_pattern_type(state, &arm.pattern, scrutinee_type);
                }
                let saved_in_guard = state.in_guard;
                state.in_guard = true;
                if let Some(guard) = &arm.guard {
                    validate_hir_expr(guard, state, true)?;
                }
                state.in_guard = saved_in_guard;
                let has_next_arm = index + 1 < arms.len();
                if let Some(scope) = state.control_flow.current_case_chain_scope_mut() {
                    scope.in_arm_body = true;
                    scope.next_arm_target = if has_next_arm { Some(index + 1) } else { None };
                }
                validate_hir_expr(&arm.body, state, false)?;
                if let Some(scope) = state.control_flow.current_case_chain_scope_mut() {
                    scope.in_arm_body = false;
                }
                pop_local_scope(state);
                previous_fallthrough_bindings =
                    if expr_contains_fallthrough(&arm.body) { collect_pattern_bound_names(&arm.pattern) } else { BTreeSet::new() };
            }
            state.control_flow.pop_case_chain();
        }
        HirExprKind::Loop { iterator, condition, body, label, pattern, .. } => {
            if let Some(iterator) = iterator {
                validate_hir_expr(iterator, state, true)?;
            }
            if let Some(condition) = condition {
                validate_hir_expr(condition, state, true)?;
            }
            if let (Some(pattern), Some(iterator)) = (pattern, iterator) {
                validate_pattern_semantics(pattern)?;
                if let Some(iterator_type) = infer_static_expr_type(iterator, state) {
                    bind_pattern_type(state, pattern, &iterator_type);
                }
            }
            push_loop_context(state, label.as_ref().map(|it| it.as_str()), value_context);
            validate_hir_block(body, state, false)?;
            pop_loop_context(state);
        }
        HirExprKind::Return(value) | HirExprKind::Yield(value) => {
            if matches!(expr.kind, HirExprKind::Yield(_)) && state.in_guard {
                return Err(HirError::ControlFlowContextViolation {
                    operation: ControlFlowOp::Yield,
                    required: RequiredContext::OutsideGuard,
                    source: control_flow_source(),
                    span: expr_diagnostic_span(expr),
                });
            }
            if matches!(expr.kind, HirExprKind::Yield(_)) {
                validate_yield_context(expr, state)?;
            }
            if let Some(value) = value {
                validate_hir_expr(value, state, true)?;
            }
            if matches!(expr.kind, HirExprKind::Return(_)) {
                validate_return_value(expr, value.as_deref(), state)?;
            }
            else {
                validate_yield_value(expr, value.as_deref(), state)?;
            }
        }
        HirExprKind::Assign { value, .. } => validate_hir_expr(value, state, true)?,
        HirExprKind::Break { label, expr: break_value } => {
            if let Some(break_value) = break_value {
                let label_str = label.as_ref().map(|it| it.as_str());
                state.control_flow.set_validating_break_value(true, label_str);
                validate_hir_expr(break_value, state, true)?;
                state.control_flow.set_validating_break_value(false, label_str);
                let break_type = infer_static_expr_type(break_value, state);
                // 先只读借用在目标 loop 上取回已有事实，再决定是否写回；否则持有可变借用时
                // 无法再做需要 `state` 的字面量判定。
                let Some((accepts_break_value, expected_type)) = resolve_break_target_mut(label.as_ref().map(|it| it.as_str()), state)
                    .map(|loop_context| (loop_context.accepts_break_value, loop_context.break_value_type.clone()))
                else {
                    return Err(HirError::LoopTargetMissing {
                        operation: LoopJumpOperation::Break,
                        carries_value: true,
                        source: control_flow_source(),
                        span: expr_diagnostic_span(break_value),
                    });
                };
                if !accepts_break_value {
                    return Err(HirError::BreakValueRejected { source: control_flow_source(), span: expr_diagnostic_span(break_value) });
                }
                if let Some(break_type) = break_type {
                    if let Some(expected_type) = &expected_type {
                        if !control_flow_value_type_compatible(expected_type, &break_type)
                            && !integer_literal_binds_to(expected_type, Some(break_value), state)
                        {
                            return Err(HirError::BreakValueTypeMismatch {
                                actual: display_type(&break_type),
                                expected: display_type(expected_type),
                                source: control_flow_source(),
                                span: expr_diagnostic_span(break_value),
                            });
                        }
                    }
                    else if let Some(loop_context) = resolve_break_target_mut(label.as_ref().map(|it| it.as_str()), state) {
                        loop_context.break_value_type = Some(break_type);
                    }
                }
            }
            else if resolve_break_target_mut(label.as_ref().map(|it| it.as_str()), state).is_none() {
                return Err(HirError::LoopTargetMissing {
                    operation: LoopJumpOperation::Break,
                    carries_value: false,
                    source: control_flow_source(),
                    span: expr_diagnostic_span(expr),
                });
            }
        }
        HirExprKind::Continue { label } => {
            if !resolve_continue_target(label.as_ref(), state) {
                return Err(HirError::LoopTargetMissing {
                    operation: LoopJumpOperation::Continue,
                    carries_value: false,
                    source: control_flow_source(),
                    span: expr_diagnostic_span(expr),
                });
            }
        }
        HirExprKind::Catch { expr, arms } => {
            validate_hir_expr(expr, state, true)?;
            let new_depth = state.control_flow.current_catch_depth() + 1;
            state.control_flow.push_catch(None, true, true);
            if let Some(scope) = state.control_flow.current_catch_scope_mut() {
                scope.depth = new_depth;
            }
            for arm in arms {
                let saved_in_guard = state.in_guard;
                state.in_guard = true;
                if let Some(scope) = state.control_flow.current_catch_scope_mut() {
                    scope.in_arm_body = false;
                }
                if let Some(guard) = &arm.guard {
                    validate_hir_expr(guard, state, true)?;
                }
                state.in_guard = false;
                if let Some(scope) = state.control_flow.current_catch_scope_mut() {
                    scope.in_arm_body = true;
                }
                validate_hir_expr(&arm.body, state, value_context)?;
                if let Some(scope) = state.control_flow.current_catch_scope_mut() {
                    scope.in_arm_body = false;
                }
                state.in_guard = saved_in_guard;
            }
            state.control_flow.pop_catch();
        }
        HirExprKind::Cast { expr, .. } => validate_hir_expr(expr, state, true)?,
        HirExprKind::Literal(_) | HirExprKind::Variable(_) | HirExprKind::Path(_) => {}
    }
    Ok(())
}

pub fn push_loop_context(state: &mut HirValidationState, label: Option<&str>, value_context: bool) {
    state.control_flow.push_loop(label.map(str::to_string), value_context);
}

pub fn pop_loop_context(state: &mut HirValidationState) {
    state.control_flow.pop_loop();
}
/// `break 'outer` / `continue 'outer` 不能跨越函数边界（lambda 是独立函数帧）。
/// 见 `ControlFlowContext::clone_for_function_boundary`。
pub fn lambda_validation_state(parent: &HirValidationState, _body: &HirBlock) -> HirValidationState {
    HirValidationState {
        in_guard: false,
        self_type_stack: parent.self_type_stack.clone(),
        return_type_stack: Vec::new(),
        local_scopes: Vec::new(),
        control_flow: parent.control_flow.clone_for_function_boundary(),
        current_function: parent.current_function.clone(),
        numeric_declarations: parent.numeric_declarations.clone(),
        boolean_declarations: parent.boolean_declarations.clone(),
    }
}

pub fn resolve_break_target_mut<'a>(label: Option<&str>, state: &'a mut HirValidationState) -> Option<&'a mut LoopValidationContext> {
    state.control_flow.resolve_loop_mut(label)
}

pub fn resolve_continue_target(label: Option<&crate::compiler::types::Identifier>, state: &HirValidationState) -> bool {
    state.control_flow.resolve_continue(label.map(|it| it.as_str()))
}

pub fn infer_static_expr_type(expr: &HirExpr, state: &HirValidationState) -> Option<ValkyrieType> {
    match &expr.kind {
        HirExprKind::Literal(literal) => infer_literal_type(literal),
        HirExprKind::Variable(identifier) => lookup_local_type(state, identifier.name.as_str()),
        HirExprKind::Block(block) => infer_static_block_type(block, state),
        HirExprKind::If { then_branch, else_branch, .. } => {
            let then_type = infer_static_block_type(then_branch, state)?;
            let else_type = else_branch.as_ref().and_then(|branch| infer_static_block_type(branch, state))?;
            if control_flow_value_type_compatible(&then_type, &else_type) { Some(then_type) } else { None }
        }
        HirExprKind::Match { scrutinee, arms } => infer_static_match_type(scrutinee, arms, state),
        HirExprKind::Case { .. } => None,
        HirExprKind::Awake(_) | HirExprKind::Yield(_) | HirExprKind::YieldFrom(_) => Some(ValkyrieType::Unit),
        HirExprKind::Await(value) | HirExprKind::BlockOn(value) => {
            let value_type = infer_static_expr_type(value, state)?;
            future_resume_type(&value_type)
        }
        HirExprKind::Call { resolved, .. } | HirExprKind::Construct { resolved, .. } => resolved.as_ref().map(|call| call.return_type.clone()),
        HirExprKind::TryPropagate { inner, .. } => infer_static_expr_type(inner, state).and_then(|ty| {
            nullable_payload_type(&ty).or_else(|| {
                if is_option_apply_type(&ty) {
                    match ty {
                        ValkyrieType::Apply(_, args) => args.first().cloned(),
                        _ => None,
                    }
                }
                else if is_result_apply_type(&ty) {
                    match ty {
                        ValkyrieType::Apply(_, args) => args.first().cloned(),
                        _ => None,
                    }
                }
                else {
                    None
                }
            })
        }),
        HirExprKind::TryScope { is_optional, is_forced, result_type, body } => {
            infer_try_scope_type(*is_optional, *is_forced, result_type.as_ref(), body, state)
        }
        _ => None,
    }
}

pub fn infer_try_scope_type(
    is_optional: bool,
    is_forced: bool,
    result_type: Option<&ValkyrieType>,
    body: &HirBlock,
    state: &HirValidationState,
) -> Option<ValkyrieType> {
    if let Some(explicit) = result_type {
        return Some(explicit.clone());
    }
    let body_type = infer_static_block_type(body, state)?;
    if is_optional {
        return Some(ValkyrieType::Nullable(Box::new(body_type)));
    }
    if is_forced {
        return Some(body_type);
    }
    Some(body_type)
}

pub fn validate_try_propagate_context(expr: &HirExpr, state: &HirValidationState) -> Result<(), HirError> {
    let HirExprKind::TryPropagate { inner, .. } = &expr.kind
    else {
        return Ok(());
    };
    let Some(operand_type) = infer_static_expr_type(inner, state)
    else {
        return Ok(());
    };
    let nullable_operand = nullable_payload_type(&operand_type).is_some();
    let option_operand = is_option_apply_type(&operand_type);
    let result_operand = is_result_apply_type(&operand_type);
    if !nullable_operand && !option_operand && !result_operand {
        return Err(HirError::TryPropagateOperandUnsupported {
            operand_type: display_type(&operand_type),
            source: control_flow_source(),
            span: expr_diagnostic_span(expr),
        });
    }
    if !state.control_flow.in_try_scope() {
        let return_type = current_return_type(state).cloned().unwrap_or(ValkyrieType::Unit);
        if nullable_operand && !is_nullable_type(&return_type) {
            return Err(HirError::TryPropagateReturnMismatch {
                protocol: PropagateProtocol::Nullable,
                return_type: display_type(&return_type),
                source: control_flow_source(),
                span: expr_diagnostic_span(expr),
            });
        }
        if option_operand && !is_option_apply_type(&return_type) {
            return Err(HirError::TryPropagateReturnMismatch {
                protocol: PropagateProtocol::Option,
                return_type: display_type(&return_type),
                source: control_flow_source(),
                span: expr_diagnostic_span(expr),
            });
        }
        if result_operand && !is_result_apply_type(&return_type) {
            return Err(HirError::TryPropagateReturnMismatch {
                protocol: PropagateProtocol::Result,
                return_type: display_type(&return_type),
                source: control_flow_source(),
                span: expr_diagnostic_span(expr),
            });
        }
    }
    Ok(())
}

pub fn resolve_contextual_type(ty: &ValkyrieType, state: &HirValidationState) -> ValkyrieType {
    match ty {
        ValkyrieType::SelfType => current_self_type(state).cloned().unwrap_or(ValkyrieType::SelfType),
        ValkyrieType::Apply(base, arguments) => ValkyrieType::Apply(
            Box::new(resolve_contextual_type(base, state)),
            arguments.iter().map(|arg| resolve_contextual_type(arg, state)).collect(),
        ),
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: function.params.iter().map(|param| resolve_contextual_type(param, state)).collect(),
            return_type: resolve_contextual_type(&function.return_type, state),
        })),
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| resolve_contextual_type(item, state)).collect()),
        ValkyrieType::Row(row) => ValkyrieType::Row(crate::compiler::types::hir::RowType {
            methods: row
                .methods
                .iter()
                .map(|method| crate::compiler::types::hir::RowMethodType {
                    name: method.name.clone(),
                    params: method.params.iter().map(|param| resolve_contextual_type(param, state)).collect(),
                    return_type: resolve_contextual_type(&method.return_type, state),
                })
                .collect(),
        }),
        ValkyrieType::Array(item) => ValkyrieType::Array(Box::new(resolve_contextual_type(item, state))),
        ValkyrieType::FixedArray { element, length } => {
            ValkyrieType::FixedArray { element: Box::new(resolve_contextual_type(element, state)), length: *length }
        }
        ValkyrieType::TypeLambda(lambda) => ValkyrieType::TypeLambda(Box::new(crate::compiler::types::hir::TypeLambda {
            params: lambda.params.clone(),
            body: resolve_contextual_type(&lambda.body, state),
        })),
        ValkyrieType::TraitObject(object) => ValkyrieType::TraitObject(crate::compiler::types::hir::TraitObject {
            trait_path: object.trait_path.clone(),
            type_arguments: object.type_arguments.iter().map(|arg| resolve_contextual_type(arg, state)).collect(),
        }),
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: resolve_contextual_type(&associated.base, state),
            name: associated.name.clone(),
            type_arguments: associated.type_arguments.iter().map(|arg| resolve_contextual_type(arg, state)).collect(),
        })),
        other => other.clone(),
    }
}

pub fn infer_static_block_type(block: &HirBlock, state: &HirValidationState) -> Option<ValkyrieType> {
    if let Some(expr) = &block.expr { infer_static_expr_type(expr, state) } else { Some(ValkyrieType::Unit) }
}

pub fn infer_static_match_type(scrutinee: &HirExpr, arms: &[HirMatchArm], state: &HirValidationState) -> Option<ValkyrieType> {
    let mut inferred_type: Option<ValkyrieType> = None;
    let scrutinee_type = infer_static_expr_type(scrutinee, state);
    for arm in arms {
        let mut arm_state = state.clone();
        push_local_scope(&mut arm_state);
        if let Some(scrutinee_type) = &scrutinee_type {
            bind_pattern_type(&mut arm_state, &arm.pattern, scrutinee_type);
        }
        let arm_type = infer_static_expr_type(&arm.body, &arm_state)?;
        if let Some(current_type) = &inferred_type {
            if !control_flow_value_type_compatible(current_type, &arm_type) {
                return None;
            }
        }
        else {
            inferred_type = Some(arm_type);
        }
    }
    inferred_type
}

pub fn infer_literal_type(literal: &HirLiteral) -> Option<ValkyrieType> {
    match literal {
        HirLiteral::Integer64(_) => Some(ValkyrieType::Integer64 { signed: true }),
        HirLiteral::Float64(_) => Some(ValkyrieType::Float64),
        HirLiteral::Bool(_) => Some(ValkyrieType::Boolean),
        HirLiteral::Unit => Some(ValkyrieType::Unit),
        HirLiteral::String(_) => None,
        // 空值没有独立名义类型；其 `T?` 由使用位置的上下文给出。
        HirLiteral::Null => None,
    }
}

pub fn validate_future_control_operand(expr: &HirExpr, value: &HirExpr, state: &HirValidationState) -> Result<(), HirError> {
    let Some(value_type) = infer_static_expr_type(value, state)
    else {
        return Ok(());
    };
    if future_resume_type(&value_type).is_some() {
        return Ok(());
    }
    let operation = match expr.kind {
        HirExprKind::Await(_) => ControlFlowOp::Await,
        HirExprKind::Awake(_) => ControlFlowOp::Awake,
        HirExprKind::BlockOn(_) => ControlFlowOp::Block,
        _ => return Ok(()),
    };
    Err(HirError::FutureControlOperandMismatch {
        operation,
        operand_type: display_type(&value_type),
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}

pub fn validate_return_value(expr: &HirExpr, value: Option<&HirExpr>, state: &HirValidationState) -> Result<(), HirError> {
    let Some(expected_raw) = current_return_type(state)
    else {
        return Ok(());
    };
    let expected_type = resolve_contextual_type(expected_raw, state);

    let Some(value) = value
    else {
        if return_without_value_compatible(&expected_type) {
            return Ok(());
        }
        return Err(HirError::ControlFlowValueRequired {
            operation: ControlFlowOp::Return,
            expected_type: display_type(&expected_type),
            source: control_flow_source(),
            span: expr_diagnostic_span(expr),
        });
    };

    let Some(actual_raw) = infer_static_expr_type(value, state)
    else {
        return Ok(());
    };
    let actual_type = resolve_contextual_type(&actual_raw, state);
    if control_flow_value_type_compatible_with_primitives(&expected_type, &actual_type, Some(state))
        || integer_literal_binds_to(&expected_type, Some(value), state)
    {
        return Ok(());
    }

    Err(HirError::ReturnValueTypeMismatch {
        actual: display_type(&actual_type),
        expected: display_type(&expected_type),
        function: state.current_function.clone(),
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}
///
/// 当函数返回类型为 `Generator<T>` / `Iterator<T>` / `Coroutine<T>` 时，
/// 函数体内所有 `yield expr` 的 `expr` 类型需与 `T` 兼容。
/// 若返回类型不是生成器协议，则跳过校验（由 `validate_yield_context` 负责 scope 校验）。
pub fn validate_yield_value(expr: &HirExpr, value: Option<&HirExpr>, state: &HirValidationState) -> Result<(), HirError> {
    let Some(expected_raw) = current_return_type(state)
    else {
        return Ok(());
    };
    let expected_return = resolve_contextual_type(expected_raw, state);
    let Some(yield_item_type) = generator_yield_type(&expected_return)
    else {
        return Ok(());
    };
    let Some(value) = value
    else {
        if matches!(yield_item_type, ValkyrieType::Unit) {
            return Ok(());
        }
        return Err(HirError::ControlFlowValueRequired {
            operation: ControlFlowOp::Yield,
            expected_type: display_type(&expected_return),
            source: control_flow_source(),
            span: expr_diagnostic_span(expr),
        });
    };
    let Some(actual_raw) = infer_static_expr_type(value, state)
    else {
        return Ok(());
    };
    let actual_type = resolve_contextual_type(&actual_raw, state);
    if control_flow_value_type_compatible(&yield_item_type, &actual_type) || integer_literal_binds_to(&yield_item_type, Some(value), state) {
        return Ok(());
    }
    Err(HirError::YieldValueTypeMismatch {
        actual: display_type(&actual_type),
        expected_return: display_type(&expected_return),
        item_type: display_type(&yield_item_type),
        function: state.current_function.clone(),
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}
///
/// `yield from` 委托另一个生成器，要求被委托生成器的 yield 元素类型与当前函数返回类型的元素类型兼容。
pub fn validate_yield_from_value(expr: &HirExpr, value: &HirExpr, state: &HirValidationState) -> Result<(), HirError> {
    let Some(expected_raw) = current_return_type(state)
    else {
        return Ok(());
    };
    let expected_return = resolve_contextual_type(expected_raw, state);
    let Some(expected_item_type) = generator_yield_type(&expected_return)
    else {
        return Ok(());
    };
    let Some(source_type_raw) = infer_static_expr_type(value, state)
    else {
        return Ok(());
    };
    let source_type = resolve_contextual_type(&source_type_raw, state);
    let Some(source_item_type) = generator_yield_type(&source_type)
    else {
        return Err(HirError::YieldFromOperandNotGenerator {
            operand_type: display_type(&source_type),
            source: control_flow_source(),
            span: expr_diagnostic_span(expr),
        });
    };
    if control_flow_value_type_compatible(&expected_item_type, &source_item_type) {
        return Ok(());
    }
    Err(HirError::YieldFromItemTypeMismatch {
        source_item: display_type(&source_item_type),
        expected_return: display_type(&expected_return),
        expected_item: display_type(&expected_item_type),
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}

pub fn validate_blocking_context(expr: &HirExpr, state: &HirValidationState) -> Result<(), HirError> {
    if !matches!(expr.kind, HirExprKind::BlockOn(_)) {
        return Ok(());
    }
    if state.control_flow.current_validating_break_value() {
        return Ok(());
    }
    if state.control_flow.in_async_scope() {
        return Err(HirError::ControlFlowContextViolation {
            operation: ControlFlowOp::Block,
            required: RequiredContext::BlockingScope,
            source: control_flow_source(),
            span: expr_diagnostic_span(expr),
        });
    }
    if state.control_flow.current_allow_blocking() {
        return Ok(());
    }
    Err(HirError::ControlFlowContextViolation {
        operation: ControlFlowOp::Block,
        required: RequiredContext::BlockingScope,
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}

pub fn validate_await_context(expr: &HirExpr, state: &HirValidationState) -> Result<(), HirError> {
    if !matches!(expr.kind, HirExprKind::Await(_)) {
        return Ok(());
    }
    if state.control_flow.current_validating_break_value() {
        return Ok(());
    }
    if state.control_flow.in_async_scope() {
        return Ok(());
    }
    Err(HirError::ControlFlowContextViolation {
        operation: ControlFlowOp::Await,
        required: RequiredContext::AsyncScope,
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}

pub fn validate_yield_context(expr: &HirExpr, state: &HirValidationState) -> Result<(), HirError> {
    let operation = match expr.kind {
        HirExprKind::Yield(_) => ControlFlowOp::Yield,
        HirExprKind::YieldFrom(_) => ControlFlowOp::YieldFrom,
        _ => {
            return Ok(());
        }
    };
    if state.control_flow.current_validating_break_value() {
        return Ok(());
    }
    if state.control_flow.current_allow_yield() {
        return Ok(());
    }
    Err(HirError::ControlFlowContextViolation {
        operation,
        required: RequiredContext::GeneratorScope,
        source: control_flow_source(),
        span: expr_diagnostic_span(expr),
    })
}
