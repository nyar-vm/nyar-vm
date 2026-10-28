use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    driver::validation::validate_semantic_module,
    hir::lowering::errors::HirError,
    types::hir::{HirBlock, HirExpr, HirExprKind, HirFunction, HirModule, HirStatementKind},
};

/// 调用合同失败必须指向真实源码身份：模块 `source_path` 是 HIR 侧唯一可用的文件事实。
///
/// `compile_source` 这类无文件身份的入口会得到空的 `source_path`，此处回落到稳定的
/// 非空模块标签，避免诊断退化成 `unknown`。
fn module_source(module: &HirModule) -> DiagnosticSource {
    if module.source_path.trim().is_empty() {
        DiagnosticSource { id: "<hir_module>".into(), revision: None }
    }
    else {
        DiagnosticSource { id: module.source_path.clone(), revision: None }
    }
}

/// 子模块沿用父模块的源码身份；只有它自己声明了 `source_path` 时才使用自己的。
fn inherited_source(module: &HirModule, inherited: &DiagnosticSource) -> DiagnosticSource {
    if module.source_path.trim().is_empty() { inherited.clone() } else { module_source(module) }
}

/// `HirExpr.span` 就是 `SourceSpan::new(SourceID, start, end)` 的源码 byte offset 半开区间，
/// 与 `DiagnosticSpan` 的 0-based 字节偏移语义一致，因此直接如实映射，不伪造字节区间。
fn expr_diagnostic_span(expr: &HirExpr) -> DiagnosticSpan {
    DiagnosticSpan { start: expr.span.get_start() as usize, end: expr.span.get_end() as usize }
}

/// 前端统一验证调用身份、枚举判别与完整语义合同，不按错误文本放行。
pub(crate) fn validate_hir_contract(hir: &HirModule) -> Result<(), HirError> {
    validate_resolved_call_contracts(hir)?;
    crate::compiler::hir::lowering::nominal_declarations::validate_enum_discriminators(hir)?;
    validate_semantic_module(hir)
}

/// Semantic MIR may only be produced from calls carrying an overload-selected
/// contract.  In particular, do not let the SSA lowerer recover a symbol or a
/// result type from source spelling, contextual type, or a backend convention.
pub(crate) fn validate_resolved_call_contracts(hir: &HirModule) -> Result<(), HirError> {
    validate_module_call_contracts(hir, &module_source(hir))
}

fn validate_module_call_contracts(hir: &HirModule, source: &DiagnosticSource) -> Result<(), HirError> {
    for submodule in &hir.submodules {
        validate_module_call_contracts(submodule, &inherited_source(submodule, source))?;
    }
    for singleton in &hir.singletons {
        for function in &singleton.methods {
            validate_function_call_contracts(function, source)?;
        }
    }
    for function in &hir.functions {
        validate_function_call_contracts(function, source)?;
    }
    for structure in &hir.structs {
        for function in &structure.methods {
            validate_function_call_contracts(function, source)?;
        }
        for property in &structure.properties {
            if let Some(function) = &property.getter {
                validate_function_call_contracts(function, source)?;
            }
            if let Some(function) = &property.setter {
                validate_function_call_contracts(function, source)?;
            }
        }
    }
    for trait_def in &hir.traits {
        for function in trait_def.methods.iter().chain(&trait_def.default_methods) {
            validate_function_call_contracts(function, source)?;
        }
    }
    for implementation in &hir.impls {
        for function in &implementation.methods {
            validate_function_call_contracts(function, source)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_function_call_contracts(function: &HirFunction, source: &DiagnosticSource) -> Result<(), HirError> {
    validate_block_call_contracts(&function.body, &function.name.to_string(), source)
}

pub(crate) fn validate_block_call_contracts(block: &HirBlock, function: &str, source: &DiagnosticSource) -> Result<(), HirError> {
    for statement in &block.statements {
        match &statement.kind {
            HirStatementKind::Let { initializer, .. } => {
                if let Some(initializer) = initializer {
                    validate_expr_call_contracts(initializer, function, source)?;
                }
            }
            HirStatementKind::Expr(expr) => validate_expr_call_contracts(expr, function, source)?,
        }
    }
    if let Some(expr) = &block.expr {
        validate_expr_call_contracts(expr, function, source)?;
    }
    Ok(())
}

#[doc(hidden)]
pub fn validate_expr_call_contracts(expr: &HirExpr, function: &str, source: &DiagnosticSource) -> Result<(), HirError> {
    match &expr.kind {
        HirExprKind::Call { callee, args, resolved, .. } => {
            if resolved.is_none() {
                let callee_name = match &callee.kind {
                    HirExprKind::Path(path) => path.to_string(),
                    _ => "<computed>".to_string(),
                };
                return Err(HirError::UnresolvedCall {
                    selector: callee_name,
                    context: function.to_string(),
                    source: source.clone(),
                    span: expr_diagnostic_span(expr),
                }
                .into());
            }
            validate_expr_call_contracts(callee, function, source)?;
            for arg in args {
                validate_expr_call_contracts(&arg.value, function, source)?;
            }
        }
        HirExprKind::Construct { path, args, resolved } => {
            if resolved.is_none() {
                return Err(HirError::UnresolvedConstructor {
                    name: path.to_string(),
                    context: function.to_string(),
                    source: source.clone(),
                    span: expr_diagnostic_span(expr),
                }
                .into());
            }
            for arg in args {
                validate_expr_call_contracts(arg, function, source)?;
            }
        }
        HirExprKind::FieldInit { value, .. }
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::TryPropagate { inner: value, .. }
        | HirExprKind::Raise(value)
        | HirExprKind::Resume(value) => validate_expr_call_contracts(value, function, source)?,
        HirExprKind::Cast { expr, .. } => validate_expr_call_contracts(expr, function, source)?,
        HirExprKind::ArrayNew { length, .. } => validate_expr_call_contracts(length, function, source)?,
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
            for item in items {
                validate_expr_call_contracts(item, function, source)?;
            }
        }
        HirExprKind::FieldAccess { object, .. } => validate_expr_call_contracts(object, function, source)?,
        HirExprKind::StoreField { object, value, .. } => {
            validate_expr_call_contracts(object, function, source)?;
            validate_expr_call_contracts(value, function, source)?;
        }
        HirExprKind::GenericApply { callee, .. } => validate_expr_call_contracts(callee, function, source)?,
        HirExprKind::Block(block) => validate_block_call_contracts(block, function, source)?,
        HirExprKind::Lambda { body, .. } => validate_block_call_contracts(body, function, source)?,
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                validate_expr_call_contracts(value, function, source)?;
            }
            for method in methods {
                validate_function_call_contracts(method, source)?;
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            validate_expr_call_contracts(condition, function, source)?;
            validate_block_call_contracts(then_branch, function, source)?;
            if let Some(block) = else_branch {
                validate_block_call_contracts(block, function, source)?;
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            validate_expr_call_contracts(scrutinee, function, source)?;
            validate_block_call_contracts(then_branch, function, source)?;
            if let Some(block) = else_branch {
                validate_block_call_contracts(block, function, source)?;
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } | HirExprKind::Catch { expr: scrutinee, arms } => {
            validate_expr_call_contracts(scrutinee, function, source)?;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    validate_expr_call_contracts(guard, function, source)?;
                }
                validate_expr_call_contracts(&arm.body, function, source)?;
            }
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                validate_expr_call_contracts(iterator, function, source)?;
            }
            if let Some(condition) = condition {
                validate_expr_call_contracts(condition, function, source)?;
            }
            validate_block_call_contracts(body, function, source)?;
        }
        HirExprKind::Return(value) | HirExprKind::Yield(value) => {
            if let Some(value) = value {
                validate_expr_call_contracts(value, function, source)?;
            }
        }
        HirExprKind::Assign { value, .. } => validate_expr_call_contracts(value, function, source)?,
        HirExprKind::Break { expr, .. } => {
            if let Some(value) = expr {
                validate_expr_call_contracts(value, function, source)?;
            }
        }
        HirExprKind::TryScope { body, .. } => validate_block_call_contracts(body, function, source)?,
        HirExprKind::With { base, updates } => {
            validate_expr_call_contracts(base, function, source)?;
            for (_, value) in updates {
                validate_expr_call_contracts(value, function, source)?;
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                validate_expr_call_contracts(arg, function, source)?;
            }
        }
        HirExprKind::Literal(_) | HirExprKind::Variable(_) | HirExprKind::Path(_) | HirExprKind::Continue { .. } | HirExprKind::Fallthrough => {
        }
    }
    Ok(())
}
