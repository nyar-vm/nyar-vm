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

/// 函数体（不含 lambda 体）是否包含 `yield` / `yield from`。
pub fn function_body_contains_yield(block: &HirBlock) -> bool {
    shallow_block_contains_yield(block)
}

/// 函数体（不含 lambda 体）是否包含 `await`。
pub fn function_body_contains_await(block: &HirBlock) -> bool {
    shallow_block_contains_await(block)
}

/// 函数体（不含 lambda 体）是否包含 `block_on`。
pub fn function_body_contains_block_on(block: &HirBlock) -> bool {
    shallow_block_contains_block_on(block)
}

/// 函数是否按 async 语义建模（当前与顶层 `await` 探测一致）。
pub fn function_is_async(block: &HirBlock) -> bool {
    function_body_contains_await(block)
}
/// 遗留名称，保留给需要区分 "may yield/await/block" 与 "has suspend effects" 的场景。
pub fn function_may_yield_or_await(block: &HirBlock) -> bool {
    block_contains_yield(block) || block_contains_await(block) || block_contains_block_on(block)
}
/// 含 lambda 体，供 `ProgramFacts.can_suspend` 与 resume frame 规划使用。
pub fn function_can_suspend(block: &HirBlock) -> bool {
    function_has_suspend_effects(block)
}
/// 含 lambda 体，供 frontend 分区与 driver payload 规划使用。
pub fn function_has_suspend_effects(block: &HirBlock) -> bool {
    function_may_yield_or_await(block) || block_contains_awake(block)
}

/// 函数体是否包含 effect-handle 相关构造（`raise` / `catch` / `resume`）。
pub fn function_can_effect_handle(block: &HirBlock) -> bool {
    block_contains_raise(block) || block_contains_catch(block) || block_contains_resume(block)
}

/// 函数是否需要生成 suspend / effect-handle 相关 MIR 片段。
pub fn function_needs_suspend_fragment(block: &HirBlock) -> bool {
    function_has_suspend_effects(block) || function_can_effect_handle(block)
}

pub fn shallow_block_contains_yield(block: &HirBlock) -> bool {
    block.statements.iter().any(shallow_statement_contains_yield) || block.expr.as_ref().is_some_and(|expr| shallow_expr_contains_yield(expr))
}

pub fn shallow_block_contains_await(block: &HirBlock) -> bool {
    block.statements.iter().any(shallow_statement_contains_await) || block.expr.as_ref().is_some_and(|expr| shallow_expr_contains_await(expr))
}

pub fn shallow_block_contains_block_on(block: &HirBlock) -> bool {
    block.statements.iter().any(shallow_statement_contains_block_on)
        || block.expr.as_ref().is_some_and(|expr| shallow_expr_contains_block_on(expr))
}

pub fn shallow_statement_contains_yield(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| shallow_expr_contains_yield(expr)),
        HirStatementKind::Expr(expr) => shallow_expr_contains_yield(expr),
    }
}

pub fn shallow_statement_contains_await(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| shallow_expr_contains_await(expr)),
        HirStatementKind::Expr(expr) => shallow_expr_contains_await(expr),
    }
}

pub fn shallow_statement_contains_block_on(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| shallow_expr_contains_block_on(expr)),
        HirStatementKind::Expr(expr) => shallow_expr_contains_block_on(expr),
    }
}

pub fn shallow_expr_contains_yield(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Yield(_) | HirExprKind::YieldFrom(_) => true,
        HirExprKind::Lambda { .. } | HirExprKind::Break { .. } => false,
        HirExprKind::Call { callee, args, .. } => {
            shallow_expr_contains_yield(callee) || args.iter().any(|arg| shallow_expr_contains_yield(&arg.value))
        }
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(shallow_expr_contains_yield)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => shallow_expr_contains_yield(length),
        HirExprKind::Fallthrough | HirExprKind::Literal(_) | HirExprKind::Variable(_) | HirExprKind::Path(_) | HirExprKind::Continue { .. } => {
            false
        }
        HirExprKind::With { base, updates } => {
            shallow_expr_contains_yield(base) || updates.iter().any(|(_, value)| shallow_expr_contains_yield(value))
        }
        HirExprKind::TryScope { body, .. } => shallow_block_contains_yield(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(shallow_expr_contains_yield),
        HirExprKind::StoreField { object, value, .. } => shallow_expr_contains_yield(object) || shallow_expr_contains_yield(value),
        HirExprKind::Block(block) => shallow_block_contains_yield(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| shallow_expr_contains_yield(value))
                || methods.iter().any(|method| shallow_block_contains_yield(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            shallow_expr_contains_yield(condition)
                || shallow_block_contains_yield(then_branch)
                || else_branch.as_ref().is_some_and(|branch| shallow_block_contains_yield(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            shallow_expr_contains_yield(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| shallow_expr_contains_yield(guard)) || shallow_expr_contains_yield(&arm.body)
                })
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| shallow_expr_contains_yield(expr))
                || condition.as_ref().is_some_and(|expr| shallow_expr_contains_yield(expr))
                || shallow_block_contains_yield(body)
        }
        HirExprKind::Catch { expr, arms } => {
            shallow_expr_contains_yield(expr)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| shallow_expr_contains_yield(guard)) || shallow_expr_contains_yield(&arm.body)
                })
        }
        HirExprKind::Return(value) => value.as_ref().is_some_and(|expr| shallow_expr_contains_yield(expr)),
    }
}

pub fn shallow_expr_contains_await(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Await(_) => true,
        HirExprKind::Lambda { .. } | HirExprKind::Break { .. } => false,
        HirExprKind::Call { callee, args, .. } => {
            shallow_expr_contains_await(callee) || args.iter().any(|arg| shallow_expr_contains_await(&arg.value))
        }
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(shallow_expr_contains_await)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => shallow_expr_contains_await(length),
        HirExprKind::Yield(_) | HirExprKind::YieldFrom(_) => false,
        HirExprKind::Fallthrough | HirExprKind::Literal(_) | HirExprKind::Variable(_) | HirExprKind::Path(_) | HirExprKind::Continue { .. } => {
            false
        }
        HirExprKind::With { base, updates } => {
            shallow_expr_contains_await(base) || updates.iter().any(|(_, value)| shallow_expr_contains_await(value))
        }
        HirExprKind::TryScope { body, .. } => shallow_block_contains_await(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(shallow_expr_contains_await),
        HirExprKind::StoreField { object, value, .. } => shallow_expr_contains_await(object) || shallow_expr_contains_await(value),
        HirExprKind::Block(block) => shallow_block_contains_await(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| shallow_expr_contains_await(value))
                || methods.iter().any(|method| shallow_block_contains_await(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            shallow_expr_contains_await(condition)
                || shallow_block_contains_await(then_branch)
                || else_branch.as_ref().is_some_and(|branch| shallow_block_contains_await(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            shallow_expr_contains_await(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| shallow_expr_contains_await(guard)) || shallow_expr_contains_await(&arm.body)
                })
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| shallow_expr_contains_await(expr))
                || condition.as_ref().is_some_and(|expr| shallow_expr_contains_await(expr))
                || shallow_block_contains_await(body)
        }
        HirExprKind::Catch { expr, arms } => {
            shallow_expr_contains_await(expr)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| shallow_expr_contains_await(guard)) || shallow_expr_contains_await(&arm.body)
                })
        }
        HirExprKind::Return(value) => value.as_ref().is_some_and(|expr| shallow_expr_contains_await(expr)),
    }
}

pub fn shallow_expr_contains_block_on(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::BlockOn(_) => true,
        HirExprKind::Lambda { .. } | HirExprKind::Break { .. } => false,
        HirExprKind::Call { callee, args, .. } => {
            shallow_expr_contains_block_on(callee) || args.iter().any(|arg| shallow_expr_contains_block_on(&arg.value))
        }
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(shallow_expr_contains_block_on)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => shallow_expr_contains_block_on(length),
        HirExprKind::Yield(_) | HirExprKind::YieldFrom(_) => false,
        HirExprKind::Fallthrough | HirExprKind::Literal(_) | HirExprKind::Variable(_) | HirExprKind::Path(_) | HirExprKind::Continue { .. } => {
            false
        }
        HirExprKind::With { base, updates } => {
            shallow_expr_contains_block_on(base) || updates.iter().any(|(_, value)| shallow_expr_contains_block_on(value))
        }
        HirExprKind::TryScope { body, .. } => shallow_block_contains_block_on(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(shallow_expr_contains_block_on),
        HirExprKind::StoreField { object, value, .. } => shallow_expr_contains_block_on(object) || shallow_expr_contains_block_on(value),
        HirExprKind::Block(block) => shallow_block_contains_block_on(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| shallow_expr_contains_block_on(value))
                || methods.iter().any(|method| shallow_block_contains_block_on(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            shallow_expr_contains_block_on(condition)
                || shallow_block_contains_block_on(then_branch)
                || else_branch.as_ref().is_some_and(|branch| shallow_block_contains_block_on(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            shallow_expr_contains_block_on(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| shallow_expr_contains_block_on(guard)) || shallow_expr_contains_block_on(&arm.body)
                })
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| shallow_expr_contains_block_on(expr))
                || condition.as_ref().is_some_and(|expr| shallow_expr_contains_block_on(expr))
                || shallow_block_contains_block_on(body)
        }
        HirExprKind::Catch { expr, arms } => {
            shallow_expr_contains_block_on(expr)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|guard| shallow_expr_contains_block_on(guard)) || shallow_expr_contains_block_on(&arm.body)
                })
        }
        HirExprKind::Return(value) => value.as_ref().is_some_and(|expr| shallow_expr_contains_block_on(expr)),
    }
}

pub fn block_contains_yield(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_yield) || block.expr.as_ref().is_some_and(|expr| expr_contains_yield(expr))
}

pub fn statement_contains_yield(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_yield(expr)),
        HirStatementKind::Expr(expr) => expr_contains_yield(expr),
    }
}

pub fn expr_contains_yield(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Yield(_) | HirExprKind::YieldFrom(_) => true,
        HirExprKind::Lambda { body, .. } => block_contains_yield(body),
        HirExprKind::Call { callee, args, .. } => expr_contains_yield(callee) || args.iter().any(|arg| expr_contains_yield(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_yield)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_yield(length),
        HirExprKind::Fallthrough | HirExprKind::Literal(_) | HirExprKind::Variable(_) | HirExprKind::Path(_) | HirExprKind::Continue { .. } => {
            false
        }
        HirExprKind::With { base, updates } => expr_contains_yield(base) || updates.iter().any(|(_, value)| expr_contains_yield(value)),
        HirExprKind::TryScope { body, .. } => block_contains_yield(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_yield),
        HirExprKind::StoreField { object, value, .. } => expr_contains_yield(object) || expr_contains_yield(value),
        HirExprKind::Block(block) => block_contains_yield(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_yield(value))
                || methods.iter().any(|method| function_body_contains_yield(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_yield(condition)
                || block_contains_yield(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_yield(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_yield(scrutinee)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_yield(guard)) || expr_contains_yield(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_yield(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_yield(expr))
                || block_contains_yield(body)
        }
        HirExprKind::Catch { expr, arms } => {
            expr_contains_yield(expr)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_yield(guard)) || expr_contains_yield(&arm.body))
        }
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } => value.as_ref().is_some_and(|expr| expr_contains_yield(expr)),
    }
}

pub fn block_contains_await(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_await) || block.expr.as_ref().is_some_and(|expr| expr_contains_await(expr))
}

pub fn block_contains_block_on(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_block_on) || block.expr.as_ref().is_some_and(|expr| expr_contains_block_on(expr))
}

pub fn block_contains_awake(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_awake) || block.expr.as_ref().is_some_and(|expr| expr_contains_awake(expr))
}

pub fn block_contains_raise(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_raise) || block.expr.as_ref().is_some_and(|expr| expr_contains_raise(expr))
}

pub fn block_contains_catch(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_catch) || block.expr.as_ref().is_some_and(|expr| expr_contains_catch(expr))
}

pub fn block_contains_resume(block: &HirBlock) -> bool {
    block.statements.iter().any(statement_contains_resume) || block.expr.as_ref().is_some_and(|expr| expr_contains_resume(expr))
}

pub fn statement_contains_raise(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_raise(expr)),
        HirStatementKind::Expr(expr) => expr_contains_raise(expr),
    }
}

pub fn statement_contains_catch(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_catch(expr)),
        HirStatementKind::Expr(expr) => expr_contains_catch(expr),
    }
}

pub fn statement_contains_resume(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_resume(expr)),
        HirStatementKind::Expr(expr) => expr_contains_resume(expr),
    }
}

pub fn expr_contains_raise(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Raise(_) => true,
        HirExprKind::Call { callee, args, .. } => expr_contains_raise(callee) || args.iter().any(|arg| expr_contains_raise(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_raise)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_raise(length),
        HirExprKind::With { base, updates } => expr_contains_raise(base) || updates.iter().any(|(_, value)| expr_contains_raise(value)),
        HirExprKind::TryScope { body, .. } => block_contains_raise(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_raise),
        HirExprKind::StoreField { object, value, .. } => expr_contains_raise(object) || expr_contains_raise(value),
        HirExprKind::Block(block) | HirExprKind::Lambda { body: block, .. } => block_contains_raise(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_raise(value)) || methods.iter().any(|method| block_contains_raise(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_raise(condition)
                || block_contains_raise(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_raise(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_raise(scrutinee)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_raise(guard)) || expr_contains_raise(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_raise(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_raise(expr))
                || block_contains_raise(body)
        }
        HirExprKind::Catch { expr, arms } => {
            expr_contains_catch(expr)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_raise(guard)) || expr_contains_raise(&arm.body))
        }
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } | HirExprKind::Yield(value) => {
            value.as_ref().is_some_and(|expr| expr_contains_raise(expr))
        }
        _ => false,
    }
}

pub fn expr_contains_catch(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Catch { .. } => true,
        HirExprKind::Call { callee, args, .. } => expr_contains_catch(callee) || args.iter().any(|arg| expr_contains_catch(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_catch)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_catch(length),
        HirExprKind::With { base, updates } => expr_contains_catch(base) || updates.iter().any(|(_, value)| expr_contains_catch(value)),
        HirExprKind::TryScope { body, .. } => block_contains_catch(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_catch),
        HirExprKind::StoreField { object, value, .. } => expr_contains_catch(object) || expr_contains_catch(value),
        HirExprKind::Block(block) | HirExprKind::Lambda { body: block, .. } => block_contains_catch(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_catch(value)) || methods.iter().any(|method| block_contains_catch(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_catch(condition)
                || block_contains_catch(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_catch(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_catch(scrutinee)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_catch(guard)) || expr_contains_catch(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_catch(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_catch(expr))
                || block_contains_catch(body)
        }
        HirExprKind::Catch { .. } => true,
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } | HirExprKind::Yield(value) => {
            value.as_ref().is_some_and(|expr| expr_contains_catch(expr))
        }
        _ => false,
    }
}

pub fn expr_contains_resume(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Resume(_) => true,
        HirExprKind::Call { callee, args, .. } => expr_contains_resume(callee) || args.iter().any(|arg| expr_contains_resume(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_resume)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Raise(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_resume(length),
        HirExprKind::With { base, updates } => expr_contains_resume(base) || updates.iter().any(|(_, value)| expr_contains_resume(value)),
        HirExprKind::TryScope { body, .. } => block_contains_resume(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_resume),
        HirExprKind::StoreField { object, value, .. } => expr_contains_resume(object) || expr_contains_resume(value),
        HirExprKind::Block(block) | HirExprKind::Lambda { body: block, .. } => block_contains_resume(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_resume(value)) || methods.iter().any(|method| block_contains_resume(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_resume(condition)
                || block_contains_resume(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_resume(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_resume(scrutinee)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_resume(guard)) || expr_contains_resume(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_resume(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_resume(expr))
                || block_contains_resume(body)
        }
        HirExprKind::Catch { expr, arms } => {
            expr_contains_resume(expr)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_resume(guard)) || expr_contains_resume(&arm.body))
        }
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } | HirExprKind::Yield(value) => {
            value.as_ref().is_some_and(|expr| expr_contains_resume(expr))
        }
        _ => false,
    }
}

pub fn statement_contains_await(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_await(expr)),
        HirStatementKind::Expr(expr) => expr_contains_await(expr),
    }
}

pub fn statement_contains_block_on(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_block_on(expr)),
        HirStatementKind::Expr(expr) => expr_contains_block_on(expr),
    }
}

pub fn statement_contains_awake(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_awake(expr)),
        HirStatementKind::Expr(expr) => expr_contains_awake(expr),
    }
}

pub fn expr_contains_await(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Await(_) => true,
        HirExprKind::Lambda { body, .. } => block_contains_await(body),
        HirExprKind::Call { callee, args, .. } => expr_contains_await(callee) || args.iter().any(|arg| expr_contains_await(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_await)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Yield(Some(length))
        | HirExprKind::YieldFrom(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_await(length),
        HirExprKind::Yield(None)
        | HirExprKind::Fallthrough
        | HirExprKind::Literal(_)
        | HirExprKind::Variable(_)
        | HirExprKind::Path(_)
        | HirExprKind::Continue { .. } => false,
        HirExprKind::With { base, updates } => expr_contains_await(base) || updates.iter().any(|(_, value)| expr_contains_await(value)),
        HirExprKind::TryScope { body, .. } => block_contains_await(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_await),
        HirExprKind::StoreField { object, value, .. } => expr_contains_await(object) || expr_contains_await(value),
        HirExprKind::Block(block) => block_contains_await(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_await(value))
                || methods.iter().any(|method| function_body_contains_await(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_await(condition)
                || block_contains_await(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_await(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_await(scrutinee)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_await(guard)) || expr_contains_await(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_await(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_await(expr))
                || block_contains_await(body)
        }
        HirExprKind::Catch { expr, arms } => {
            expr_contains_await(expr)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_await(guard)) || expr_contains_await(&arm.body))
        }
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } => value.as_ref().is_some_and(|expr| expr_contains_await(expr)),
    }
}

pub fn expr_contains_block_on(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::BlockOn(_) => true,
        HirExprKind::Lambda { body, .. } => block_contains_block_on(body),
        HirExprKind::Call { callee, args, .. } => expr_contains_block_on(callee) || args.iter().any(|arg| expr_contains_block_on(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_block_on)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::Yield(Some(length))
        | HirExprKind::YieldFrom(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_block_on(length),
        HirExprKind::Yield(None)
        | HirExprKind::Fallthrough
        | HirExprKind::Literal(_)
        | HirExprKind::Variable(_)
        | HirExprKind::Path(_)
        | HirExprKind::Continue { .. } => false,
        HirExprKind::With { base, updates } => expr_contains_block_on(base) || updates.iter().any(|(_, value)| expr_contains_block_on(value)),
        HirExprKind::TryScope { body, .. } => block_contains_block_on(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_block_on),
        HirExprKind::StoreField { object, value, .. } => expr_contains_block_on(object) || expr_contains_block_on(value),
        HirExprKind::Block(block) => block_contains_block_on(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_block_on(value))
                || methods.iter().any(|method| function_body_contains_block_on(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_block_on(condition)
                || block_contains_block_on(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_block_on(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_block_on(scrutinee)
                || arms
                    .iter()
                    .any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_block_on(guard)) || expr_contains_block_on(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_block_on(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_block_on(expr))
                || block_contains_block_on(body)
        }
        HirExprKind::Catch { expr, arms } => {
            expr_contains_block_on(expr)
                || arms
                    .iter()
                    .any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_block_on(guard)) || expr_contains_block_on(&arm.body))
        }
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } => value.as_ref().is_some_and(|expr| expr_contains_block_on(expr)),
    }
}

pub fn expr_contains_awake(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Awake(_) => true,
        HirExprKind::Lambda { body, .. } => block_contains_awake(body),
        HirExprKind::Call { callee, args, .. } => expr_contains_awake(callee) || args.iter().any(|arg| expr_contains_awake(&arg.value)),
        HirExprKind::Construct { args, .. } | HirExprKind::ArrayLiteral { items: args } | HirExprKind::TupleLiteral { items: args } => {
            args.iter().any(expr_contains_awake)
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Yield(Some(length))
        | HirExprKind::YieldFrom(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_awake(length),
        HirExprKind::Yield(None)
        | HirExprKind::Fallthrough
        | HirExprKind::Literal(_)
        | HirExprKind::Variable(_)
        | HirExprKind::Path(_)
        | HirExprKind::Continue { .. } => false,
        HirExprKind::With { base, updates } => expr_contains_awake(base) || updates.iter().any(|(_, value)| expr_contains_awake(value)),
        HirExprKind::TryScope { body, .. } => block_contains_awake(body),
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_awake),
        HirExprKind::StoreField { object, value, .. } => expr_contains_awake(object) || expr_contains_awake(value),
        HirExprKind::Block(block) => block_contains_awake(block),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_awake(value)) || methods.iter().any(|method| block_contains_awake(&method.body))
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_awake(condition)
                || block_contains_awake(then_branch)
                || else_branch.as_ref().is_some_and(|branch| block_contains_awake(branch))
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_awake(scrutinee)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_awake(guard)) || expr_contains_awake(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_awake(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_awake(expr))
                || block_contains_awake(body)
        }
        HirExprKind::Catch { expr, arms } => {
            expr_contains_awake(expr)
                || arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_awake(guard)) || expr_contains_awake(&arm.body))
        }
        HirExprKind::Return(value) | HirExprKind::Break { expr: value, .. } => value.as_ref().is_some_and(|expr| expr_contains_awake(expr)),
    }
}
