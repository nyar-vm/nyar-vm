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

use crate::compiler::hir::control_flow_validation::state::{HirValidationState, LocalBinding};
pub fn push_local_scope(state: &mut HirValidationState) {
    state.local_scopes.push(BTreeMap::new());
}

pub fn pop_local_scope(state: &mut HirValidationState) {
    let _ = state.local_scopes.pop();
}

pub fn push_return_type(state: &mut HirValidationState, ty: ValkyrieType) {
    state.return_type_stack.push(ty);
}

pub fn pop_return_type(state: &mut HirValidationState) {
    let _ = state.return_type_stack.pop();
}

pub fn current_return_type(state: &HirValidationState) -> Option<&ValkyrieType> {
    state.return_type_stack.last()
}

pub fn push_self_type(state: &mut HirValidationState, ty: ValkyrieType) {
    state.self_type_stack.push(ty);
}

pub fn pop_self_type(state: &mut HirValidationState) {
    let _ = state.self_type_stack.pop();
}

pub fn current_self_type(state: &HirValidationState) -> Option<&ValkyrieType> {
    state.self_type_stack.last()
}

/// 按完整声明事实登记一个名字绑定。
pub fn bind_local(state: &mut HirValidationState, name: &str, binding: LocalBinding) {
    if let Some(scope) = state.local_scopes.last_mut() {
        scope.insert(name.to_string(), binding);
    }
}

/// 登记形参绑定：只登记可变性事实。
///
/// 规范对形参名重绑定沉默（`scope.md` 的只读承诺针对原地修改），本轮不在体内赋值上收紧，
/// 因此形参的 `rejects_assignment` 恒为 false。
pub fn bind_parameter_local(state: &mut HirValidationState, name: &str, ty: ValkyrieType, is_mutable: bool) {
    bind_local(state, name, LocalBinding { ty, is_mutable, rejects_assignment: false });
}

/// pattern 绑定登记时携带的声明面可变性事实。
#[derive(Debug, Clone, Copy)]
pub struct PatternBindingFacts {
    /// 声明面是否可变。
    pub is_mutable: bool,
    /// 体内赋值是否应被拒绝（仅不可变且带初始化的 `let` 为 true）。
    pub rejects_assignment: bool,
}

impl PatternBindingFacts {
    /// `case` / `if let` / loop pattern 等非 `let` 声明位的默认事实：登记但拒绝收紧。
    pub const LENIENT: Self = Self { is_mutable: false, rejects_assignment: false };
}

pub fn bind_pattern_type(state: &mut HirValidationState, pattern: &HirPattern, ty: &ValkyrieType) {
    bind_pattern_type_with_hint(state, pattern, Some(ty), PatternBindingFacts::LENIENT);
}

/// `let` 声明位专用登记：携带该 `let` 的可变性与「不可变且带初始化」的拒绝事实。
pub fn bind_pattern_type_with_facts(state: &mut HirValidationState, pattern: &HirPattern, ty: &ValkyrieType, facts: PatternBindingFacts) {
    bind_pattern_type_with_hint(state, pattern, Some(ty), facts);
}

pub fn validate_pattern_semantics(pattern: &HirPattern) -> Result<(), HirError> {
    match pattern {
        HirPattern::Name(name) => {
            if is_ambiguous_bare_name_pattern(name) {
                return Err(HirError::AmbiguousBareNamePattern {
                    name: render_pattern_name(name),
                    source: control_flow_source(),
                    span: DiagnosticSpan { start: 0, end: 0 },
                });
            }
        }
        HirPattern::Tuple(items) | HirPattern::Or(items) => {
            for item in items {
                validate_pattern_semantics(item)?;
            }
        }
        HirPattern::Extractor(extractor) => match extractor {
            crate::compiler::types::hir::HirExtractorPattern::Array { prefix, suffix, .. } => {
                for item in prefix {
                    validate_pattern_semantics(item)?;
                }
                for item in suffix {
                    validate_pattern_semantics(item)?;
                }
            }
            crate::compiler::types::hir::HirExtractorPattern::Constructor { fields, .. } => {
                for field in fields {
                    validate_pattern_semantics(field)?;
                }
            }
        },
        HirPattern::Object { fields, .. } => {
            for (_, field_pattern) in fields {
                validate_pattern_semantics(field_pattern)?;
            }
        }
        HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
            validate_pattern_semantics(pattern)?;
        }
        _ => {}
    }
    Ok(())
}

pub fn is_ambiguous_bare_name_pattern(name: &NamePath) -> bool {
    name.parts().len() == 1
        && name.parts().first().and_then(|identifier| identifier.as_str().chars().next()).is_some_and(|ch| ch.is_lowercase())
}

pub fn render_pattern_name(name: &NamePath) -> String {
    name.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>().join("::")
}

pub fn bind_pattern_type_with_hint(
    state: &mut HirValidationState,
    pattern: &HirPattern,
    ty: Option<&ValkyrieType>,
    facts: PatternBindingFacts,
) {
    match pattern {
        HirPattern::Variable(identifier) => {
            bind_local(state, identifier.name.as_str(), binding_for(ty, facts));
        }
        HirPattern::TypedBind { identifier, ty: pattern_type } => {
            let explicit = pattern_type.parts().last().cloned().map(ValkyrieType::Named);
            let ty = explicit.or_else(|| ty.cloned()).unwrap_or(ValkyrieType::AutoType);
            bind_local(
                state,
                identifier.name.as_str(),
                LocalBinding { ty, is_mutable: facts.is_mutable, rejects_assignment: facts.rejects_assignment },
            );
        }
        HirPattern::Tuple(items) => {
            if let Some(ValkyrieType::Tuple(types)) = ty {
                for (item, item_type) in items.iter().zip(types.iter()) {
                    bind_pattern_type_with_hint(state, item, Some(item_type), facts);
                }
            }
            for item in items.iter().skip(match ty {
                Some(ValkyrieType::Tuple(types)) => types.len(),
                _ => 0,
            }) {
                bind_pattern_type_with_hint(state, item, None, facts);
            }
        }
        HirPattern::Extractor(extractor) => match extractor {
            crate::compiler::types::hir::HirExtractorPattern::Array { prefix, rest, suffix, .. } => {
                let item_hint = match ty {
                    Some(ValkyrieType::Array(item)) => Some(item.as_ref()),
                    _ => None,
                };
                for item in prefix {
                    bind_pattern_type_with_hint(state, item, item_hint, facts);
                }
                if let Some(rest) = rest {
                    let rest_ty = item_hint.cloned().map(|item| ValkyrieType::Array(Box::new(item))).unwrap_or(ValkyrieType::AutoType);
                    bind_local(
                        state,
                        rest.name.as_str(),
                        LocalBinding { ty: rest_ty, is_mutable: facts.is_mutable, rejects_assignment: facts.rejects_assignment },
                    );
                }
                for item in suffix {
                    bind_pattern_type_with_hint(state, item, item_hint, facts);
                }
            }
            crate::compiler::types::hir::HirExtractorPattern::Constructor { fields, .. } => {
                for field in fields {
                    bind_pattern_type_with_hint(state, field, None, facts);
                }
            }
        },
        HirPattern::Object { fields, rest, .. } => {
            for (_, field_pattern) in fields {
                bind_pattern_type_with_hint(state, field_pattern, None, facts);
            }
            if let Some(rest) = rest {
                bind_local(state, rest.name.as_str(), binding_for(ty, facts));
            }
        }
        HirPattern::Or(patterns) => {
            for pattern in patterns {
                bind_pattern_type_with_hint(state, pattern, ty, facts);
            }
        }
        HirPattern::Bind { identifier, pattern } => {
            bind_pattern_type_with_hint(state, pattern, ty, facts);
            bind_local(state, identifier.name.as_str(), binding_for(ty, facts));
        }
        HirPattern::Mut(pattern) => {
            // pattern 级 `mut` 是声明面可变事实：其下绑定一律按可变登记，不再收紧赋值。
            bind_pattern_type_with_hint(state, pattern, ty, PatternBindingFacts { is_mutable: true, rejects_assignment: false });
        }
        HirPattern::Pin { mutable, pattern } => {
            // `pin` 本身只约束值不被移动；可变性取 `pin mut` 与声明位事实的并，
            // `pin mut` 视同可变绑定。
            let inner = if *mutable { PatternBindingFacts { is_mutable: true, rejects_assignment: false } } else { facts };
            bind_pattern_type_with_hint(state, pattern, ty, inner);
        }
        _ => {}
    }
}

/// 以 hint 类型与声明事实构造一个 `LocalBinding`。
fn binding_for(ty: Option<&ValkyrieType>, facts: PatternBindingFacts) -> LocalBinding {
    LocalBinding {
        ty: ty.cloned().unwrap_or(ValkyrieType::AutoType),
        is_mutable: facts.is_mutable,
        rejects_assignment: facts.rejects_assignment,
    }
}

pub fn collect_pattern_bound_names(pattern: &HirPattern) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    collect_pattern_bound_names_into(pattern, &mut names);
    names
}

pub fn collect_pattern_bound_names_into(pattern: &HirPattern, names: &mut BTreeSet<String>) {
    match pattern {
        HirPattern::Variable(identifier) => {
            names.insert(identifier.name.to_string());
        }
        HirPattern::TypedBind { identifier, .. } => {
            names.insert(identifier.name.to_string());
        }
        HirPattern::Tuple(items) => {
            for item in items {
                collect_pattern_bound_names_into(item, names);
            }
        }
        HirPattern::Extractor(extractor) => match extractor {
            crate::compiler::types::hir::HirExtractorPattern::Array { prefix, rest, suffix, .. } => {
                for item in prefix {
                    collect_pattern_bound_names_into(item, names);
                }
                if let Some(rest) = rest {
                    names.insert(rest.name.to_string());
                }
                for item in suffix {
                    collect_pattern_bound_names_into(item, names);
                }
            }
            crate::compiler::types::hir::HirExtractorPattern::Constructor { fields, .. } => {
                for field in fields {
                    collect_pattern_bound_names_into(field, names);
                }
            }
        },
        HirPattern::Object { fields, rest, .. } => {
            for (_, field_pattern) in fields {
                collect_pattern_bound_names_into(field_pattern, names);
            }
            if let Some(rest) = rest {
                names.insert(rest.name.to_string());
            }
        }
        HirPattern::Or(patterns) => {
            for pattern in patterns {
                collect_pattern_bound_names_into(pattern, names);
            }
        }
        HirPattern::Bind { identifier, pattern } => {
            names.insert(identifier.name.to_string());
            collect_pattern_bound_names_into(pattern, names);
        }
        HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
            collect_pattern_bound_names_into(pattern, names);
        }
        _ => {}
    }
}

pub fn detect_case_fallthrough_binding_leak(
    arm: &HirMatchArm,
    previous_fallthrough_bindings: &BTreeSet<String>,
    state: &HirValidationState,
) -> Option<String> {
    if previous_fallthrough_bindings.is_empty() {
        return None;
    }

    let current_bindings = collect_pattern_bound_names(&arm.pattern);
    let mut referenced = BTreeSet::new();
    if let Some(guard) = &arm.guard {
        collect_expr_variable_names(guard, &mut referenced);
    }
    collect_expr_variable_names(&arm.body, &mut referenced);

    referenced.into_iter().find(|name| {
        previous_fallthrough_bindings.contains(name) && !current_bindings.contains(name) && lookup_local_type(state, name).is_none()
    })
}

pub fn collect_expr_variable_names(expr: &HirExpr, names: &mut BTreeSet<String>) {
    match &expr.kind {
        HirExprKind::Variable(identifier) => {
            names.insert(identifier.name.to_string());
        }
        HirExprKind::Call { callee, args, .. } => {
            collect_expr_variable_names(callee, names);
            for arg in args {
                collect_expr_variable_names(&arg.value, names);
            }
        }
        HirExprKind::Construct { args, .. }
        | HirExprKind::ArrayLiteral { items: args }
        | HirExprKind::TupleLiteral { items: args }
        | HirExprKind::CollectionInit { items: args, .. } => {
            for arg in args {
                collect_expr_variable_names(arg, names);
            }
        }
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Yield(Some(length))
        | HirExprKind::YieldFrom(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => collect_expr_variable_names(length, names),
        HirExprKind::Yield(None)
        | HirExprKind::Fallthrough
        | HirExprKind::Literal(_)
        | HirExprKind::Path(_)
        | HirExprKind::FunctionReference(_)
        | HirExprKind::Continue { .. } => {}
        HirExprKind::With { base, updates } => {
            collect_expr_variable_names(base, names);
            for (_, value) in updates {
                collect_expr_variable_names(value, names);
            }
        }
        HirExprKind::TryScope { body, .. } => {
            for statement in &body.statements {
                collect_statement_variable_names(statement, names);
            }
            if let Some(expr) = &body.expr {
                collect_expr_variable_names(expr, names);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                collect_expr_variable_names(arg, names);
            }
        }
        HirExprKind::StoreField { object, value, .. } => {
            collect_expr_variable_names(object, names);
            collect_expr_variable_names(value, names);
        }
        HirExprKind::Block(block) => {
            for statement in &block.statements {
                collect_statement_variable_names(statement, names);
            }
            if let Some(expr) = &block.expr {
                collect_expr_variable_names(expr, names);
            }
        }
        HirExprKind::Lambda { body, .. } => {
            for statement in &body.statements {
                collect_statement_variable_names(statement, names);
            }
            if let Some(expr) = &body.expr {
                collect_expr_variable_names(expr, names);
            }
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                collect_expr_variable_names(value, names);
            }
            for method in methods {
                for statement in &method.body.statements {
                    collect_statement_variable_names(statement, names);
                }
                if let Some(expr) = &method.body.expr {
                    collect_expr_variable_names(expr, names);
                }
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            collect_expr_variable_names(condition, names);
            for statement in &then_branch.statements {
                collect_statement_variable_names(statement, names);
            }
            if let Some(expr) = &then_branch.expr {
                collect_expr_variable_names(expr, names);
            }
            if let Some(else_branch) = else_branch {
                for statement in &else_branch.statements {
                    collect_statement_variable_names(statement, names);
                }
                if let Some(expr) = &else_branch.expr {
                    collect_expr_variable_names(expr, names);
                }
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            collect_expr_variable_names(scrutinee, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_variable_names(guard, names);
                }
                collect_expr_variable_names(&arm.body, names);
            }
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                collect_expr_variable_names(iterator, names);
            }
            if let Some(condition) = condition {
                collect_expr_variable_names(condition, names);
            }
            for statement in &body.statements {
                collect_statement_variable_names(statement, names);
            }
            if let Some(expr) = &body.expr {
                collect_expr_variable_names(expr, names);
            }
        }
        HirExprKind::Return(Some(value)) | HirExprKind::Break { expr: Some(value), .. } => collect_expr_variable_names(value, names),
        HirExprKind::Return(None) | HirExprKind::Break { expr: None, .. } | HirExprKind::Catch { .. } => {}
    }
}

pub fn collect_statement_variable_names(statement: &HirStatement, names: &mut BTreeSet<String>) {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_expr_variable_names(initializer, names);
            }
        }
        HirStatementKind::Expr(expr) => collect_expr_variable_names(expr, names),
    }
}

pub fn expr_contains_fallthrough(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::Fallthrough => true,
        HirExprKind::Call { callee, args, .. } => {
            expr_contains_fallthrough(callee) || args.iter().any(|arg| expr_contains_fallthrough(&arg.value))
        }
        HirExprKind::Construct { args, .. }
        | HirExprKind::ArrayLiteral { items: args }
        | HirExprKind::TupleLiteral { items: args }
        | HirExprKind::CollectionInit { items: args, .. } => args.iter().any(expr_contains_fallthrough),
        HirExprKind::ArrayNew { length, .. }
        | HirExprKind::FieldInit { value: length, .. }
        | HirExprKind::FieldAccess { object: length, .. }
        | HirExprKind::Await(length)
        | HirExprKind::Awake(length)
        | HirExprKind::BlockOn(length)
        | HirExprKind::Yield(Some(length))
        | HirExprKind::YieldFrom(length)
        | HirExprKind::Raise(length)
        | HirExprKind::Resume(length)
        | HirExprKind::TryPropagate { inner: length, .. }
        | HirExprKind::Assign { value: length, .. }
        | HirExprKind::GenericApply { callee: length, .. }
        | HirExprKind::Cast { expr: length, .. } => expr_contains_fallthrough(length),
        HirExprKind::Yield(None)
        | HirExprKind::Literal(_)
        | HirExprKind::Variable(_)
        | HirExprKind::Path(_)
        | HirExprKind::FunctionReference(_)
        | HirExprKind::Continue { .. } => false,
        HirExprKind::With { base, updates } => {
            expr_contains_fallthrough(base) || updates.iter().any(|(_, value)| expr_contains_fallthrough(value))
        }
        HirExprKind::TryScope { body, .. } => {
            body.statements.iter().any(statement_contains_fallthrough) || body.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
        }
        HirExprKind::SuperCall { args, .. } => args.iter().any(expr_contains_fallthrough),
        HirExprKind::StoreField { object, value, .. } => expr_contains_fallthrough(object) || expr_contains_fallthrough(value),
        HirExprKind::Block(block) => {
            block.statements.iter().any(statement_contains_fallthrough)
                || block.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
        }
        HirExprKind::Lambda { body, .. } => {
            body.statements.iter().any(statement_contains_fallthrough) || body.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            fields.iter().any(|(_, value)| expr_contains_fallthrough(value))
                || methods.iter().any(|method| {
                    method.body.statements.iter().any(statement_contains_fallthrough)
                        || method.body.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
                })
        }
        HirExprKind::If { condition, then_branch, else_branch } | HirExprKind::IfLet { scrutinee: condition, then_branch, else_branch, .. } => {
            expr_contains_fallthrough(condition)
                || then_branch.statements.iter().any(statement_contains_fallthrough)
                || then_branch.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
                || else_branch.as_ref().is_some_and(|branch| {
                    branch.statements.iter().any(statement_contains_fallthrough)
                        || branch.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
                })
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            expr_contains_fallthrough(scrutinee)
                || arms
                    .iter()
                    .any(|arm| arm.guard.as_ref().is_some_and(|guard| expr_contains_fallthrough(guard)) || expr_contains_fallthrough(&arm.body))
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            iterator.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
                || condition.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
                || body.statements.iter().any(statement_contains_fallthrough)
                || body.expr.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr))
        }
        HirExprKind::Return(Some(value)) | HirExprKind::Break { expr: Some(value), .. } => expr_contains_fallthrough(value),
        HirExprKind::Return(None) | HirExprKind::Break { expr: None, .. } | HirExprKind::Catch { .. } => false,
    }
}

pub fn statement_contains_fallthrough(statement: &HirStatement) -> bool {
    match &statement.kind {
        HirStatementKind::Let { initializer, .. } => initializer.as_ref().is_some_and(|expr| expr_contains_fallthrough(expr)),
        HirStatementKind::Expr(expr) => expr_contains_fallthrough(expr),
    }
}

pub fn lookup_local_type(state: &HirValidationState, name: &str) -> Option<ValkyrieType> {
    lookup_local(state, name).map(|binding| binding.ty.clone())
}

/// 取回局部绑定的完整声明事实（类型 + 可变性 + 赋值拒绝）。
pub fn lookup_local<'a>(state: &'a HirValidationState, name: &str) -> Option<&'a LocalBinding> {
    state.local_scopes.iter().rev().find_map(|scope| scope.get(name))
}
