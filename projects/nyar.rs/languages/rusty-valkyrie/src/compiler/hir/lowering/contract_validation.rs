use nyar_types::{DiagnosticArg, DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    diagnostics::pending::{self, CompileDiagnostic},
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
///
/// 同一编译单元内**全部**未解析调用/构造在此收集并记入诊断暂存汇（多诊断报告），
/// 控制流保持返回首个错误：编译在阶段边界仍然 fail-closed，不放宽任何语义检查。
pub(crate) fn validate_resolved_call_contracts(hir: &HirModule) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_module_call_contracts(hir, &module_source(hir), &mut errors);
    let mut errors = errors.into_iter();
    let Some(first) = errors.next()
    else {
        return Ok(());
    };
    for error in std::iter::once(&first).chain(errors.as_slice()) {
        if let Some(diagnostic) = compile_diagnostic_from_call_error(error) {
            pending::push_compile_diagnostic(diagnostic);
        }
    }
    Err(first)
}

/// 把调用合同失败的 `HirError` 投影为多诊断报告条目。
///
/// 只处理本收集器会生产的两个变体；`code` 复用 `HirError::code` 的唯一事实源，
/// 消息是不带定位后缀的展示文本（定位由渲染层从 `source` + `span` 展开）。
fn compile_diagnostic_from_call_error(error: &HirError) -> Option<CompileDiagnostic> {
    let row = match error {
        HirError::UnresolvedCall { selector, context, source, span } => DiagnosticRow::new(
            error.code(),
            DiagnosticSeverity::Error,
            DiagnosticPhase::Hir,
            DiagnosticOwner::frontend("rusty-valkyrie"),
            source.clone(),
            span.clone(),
        )
        .with_arg("selector", DiagnosticArg::String(selector.clone()))
        .with_arg("context", DiagnosticArg::String(context.clone())),
        HirError::UnresolvedConstructor { name, context, source, span } => DiagnosticRow::new(
            error.code(),
            DiagnosticSeverity::Error,
            DiagnosticPhase::Hir,
            DiagnosticOwner::frontend("rusty-valkyrie"),
            source.clone(),
            span.clone(),
        )
        .with_arg("name", DiagnosticArg::Aggregate(name.clone()))
        .with_arg("context", DiagnosticArg::String(context.clone())),
        _ => return None,
    };
    let message = match error {
        HirError::UnresolvedCall { selector, context, .. } => format!("unresolved call `{selector}` in {context}"),
        HirError::UnresolvedConstructor { name, context, .. } => format!("unresolved constructor `{name}` in {context}"),
        _ => unreachable!("the row match above already filtered to call diagnostics"),
    };
    Some(CompileDiagnostic::new(row, message))
}

fn collect_module_call_contracts(hir: &HirModule, source: &DiagnosticSource, errors: &mut Vec<HirError>) {
    for submodule in &hir.submodules {
        collect_module_call_contracts(submodule, &inherited_source(submodule, source), errors);
    }
    for singleton in &hir.singletons {
        for function in &singleton.methods {
            collect_function_call_contracts(function, source, errors);
        }
    }
    for function in &hir.functions {
        collect_function_call_contracts(function, source, errors);
    }
    for structure in &hir.structs {
        for function in &structure.methods {
            collect_function_call_contracts(function, source, errors);
        }
        for property in &structure.properties {
            if let Some(function) = &property.getter {
                collect_function_call_contracts(function, source, errors);
            }
            if let Some(function) = &property.setter {
                collect_function_call_contracts(function, source, errors);
            }
        }
    }
    for trait_def in &hir.traits {
        for function in trait_def.methods.iter().chain(&trait_def.default_methods) {
            collect_function_call_contracts(function, source, errors);
        }
    }
    for implementation in &hir.impls {
        for function in &implementation.methods {
            collect_function_call_contracts(function, source, errors);
        }
    }
}

// 仅保留给 `lowering::mod` 的 pub(crate) 门面 re-export；收集式遍历不再经此入口。
#[allow(dead_code)]
pub(crate) fn validate_function_call_contracts(function: &HirFunction, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_function_call_contracts(function, source, &mut errors);
    errors.into_iter().next().map_or(Ok(()), Err)
}

fn collect_function_call_contracts(function: &HirFunction, source: &DiagnosticSource, errors: &mut Vec<HirError>) {
    collect_block_call_contracts(&function.body, &function.name.to_string(), source, errors);
}

// 仅保留给 `lowering::mod` 的 pub(crate) 门面 re-export；收集式遍历不再经此入口。
#[allow(dead_code)]
pub(crate) fn validate_block_call_contracts(block: &HirBlock, function: &str, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_block_call_contracts(block, function, source, &mut errors);
    errors.into_iter().next().map_or(Ok(()), Err)
}

fn collect_block_call_contracts(block: &HirBlock, function: &str, source: &DiagnosticSource, errors: &mut Vec<HirError>) {
    for statement in &block.statements {
        match &statement.kind {
            HirStatementKind::Let { initializer, .. } => {
                if let Some(initializer) = initializer {
                    collect_expr_call_contracts(initializer, function, source, errors);
                }
            }
            HirStatementKind::Expr(expr) => collect_expr_call_contracts(expr, function, source, errors),
        }
    }
    if let Some(expr) = &block.expr {
        collect_expr_call_contracts(expr, function, source, errors);
    }
}

#[doc(hidden)]
pub fn validate_expr_call_contracts(expr: &HirExpr, function: &str, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_expr_call_contracts(expr, function, source, &mut errors);
    errors.into_iter().next().map_or(Ok(()), Err)
}

/// 前序遍历表达式树，收集全部未解析调用/构造；顺序与旧的首次失败遍历一致，
/// 因此收集版返回的首个错误与旧行为逐字节相同。
fn collect_expr_call_contracts(expr: &HirExpr, function: &str, source: &DiagnosticSource, errors: &mut Vec<HirError>) {
    match &expr.kind {
        HirExprKind::Call { callee, args, resolved, .. } => {
            if resolved.is_none() {
                let callee_name = match &callee.kind {
                    HirExprKind::Path(path) => path.to_string(),
                    _ => "<computed>".to_string(),
                };
                errors.push(HirError::UnresolvedCall {
                    selector: callee_name,
                    context: function.to_string(),
                    source: source.clone(),
                    span: expr_diagnostic_span(expr),
                });
            }
            collect_expr_call_contracts(callee, function, source, errors);
            for arg in args {
                collect_expr_call_contracts(&arg.value, function, source, errors);
            }
        }
        HirExprKind::Construct { path, args, resolved } => {
            if resolved.is_none() {
                errors.push(HirError::UnresolvedConstructor {
                    name: path.to_string(),
                    context: function.to_string(),
                    source: source.clone(),
                    span: expr_diagnostic_span(expr),
                });
            }
            for arg in args {
                collect_expr_call_contracts(arg, function, source, errors);
            }
        }
        HirExprKind::FieldInit { value, .. }
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::TryPropagate { inner: value, .. }
        | HirExprKind::Raise(value)
        | HirExprKind::Resume(value) => collect_expr_call_contracts(value, function, source, errors),
        HirExprKind::Cast { expr, .. } => collect_expr_call_contracts(expr, function, source, errors),
        HirExprKind::ArrayNew { length, .. } => collect_expr_call_contracts(length, function, source, errors),
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } | HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                collect_expr_call_contracts(item, function, source, errors);
            }
        }
        HirExprKind::FieldAccess { object, .. } => collect_expr_call_contracts(object, function, source, errors),
        HirExprKind::StoreField { object, value, .. } => {
            collect_expr_call_contracts(object, function, source, errors);
            collect_expr_call_contracts(value, function, source, errors);
        }
        HirExprKind::GenericApply { callee, .. } => collect_expr_call_contracts(callee, function, source, errors),
        HirExprKind::Block(block) => collect_block_call_contracts(block, function, source, errors),
        HirExprKind::Lambda { body, .. } => collect_block_call_contracts(body, function, source, errors),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                collect_expr_call_contracts(value, function, source, errors);
            }
            for method in methods {
                collect_function_call_contracts(method, source, errors);
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            collect_expr_call_contracts(condition, function, source, errors);
            collect_block_call_contracts(then_branch, function, source, errors);
            if let Some(block) = else_branch {
                collect_block_call_contracts(block, function, source, errors);
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            collect_expr_call_contracts(scrutinee, function, source, errors);
            collect_block_call_contracts(then_branch, function, source, errors);
            if let Some(block) = else_branch {
                collect_block_call_contracts(block, function, source, errors);
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } | HirExprKind::Catch { expr: scrutinee, arms } => {
            collect_expr_call_contracts(scrutinee, function, source, errors);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_call_contracts(guard, function, source, errors);
                }
                collect_expr_call_contracts(&arm.body, function, source, errors);
            }
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                collect_expr_call_contracts(iterator, function, source, errors);
            }
            if let Some(condition) = condition {
                collect_expr_call_contracts(condition, function, source, errors);
            }
            collect_block_call_contracts(body, function, source, errors);
        }
        HirExprKind::Return(value) | HirExprKind::Yield(value) => {
            if let Some(value) = value {
                collect_expr_call_contracts(value, function, source, errors);
            }
        }
        HirExprKind::Assign { value, .. } => collect_expr_call_contracts(value, function, source, errors),
        HirExprKind::Break { expr, .. } => {
            if let Some(value) = expr {
                collect_expr_call_contracts(value, function, source, errors);
            }
        }
        HirExprKind::TryScope { body, .. } => collect_block_call_contracts(body, function, source, errors),
        HirExprKind::With { base, updates } => {
            collect_expr_call_contracts(base, function, source, errors);
            for (_, value) in updates {
                collect_expr_call_contracts(value, function, source, errors);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                collect_expr_call_contracts(arg, function, source, errors);
            }
        }
        // 函数引用没有子表达式需要递归；身份缺失由 MIR  lowering 的 fail-closed 处理。
        HirExprKind::Literal(_)
        | HirExprKind::Variable(_)
        | HirExprKind::Path(_)
        | HirExprKind::FunctionReference(_)
        | HirExprKind::Continue { .. }
        | HirExprKind::Fallthrough => {}
    }
}
