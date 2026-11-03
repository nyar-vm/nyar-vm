use nyar_types::{DiagnosticArg, DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    diagnostics::pending::{self, CompileDiagnostic},
    driver::validation::validate_semantic_module,
    hir::{lowering::errors::HirError, overload::has_unresolved_generic_arguments},
    types::hir::{HirBlock, HirCallableDomain, HirExpr, HirExprKind, HirFunction, HirModule, HirResolvedCall, HirStatementKind},
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

/// 表达式自带的 `SourceID` 经 `diagnostic_sources` 表回查真实文件身份；
/// 表内缺失（单文件编译、旧缓存模块）时回退到调用方继承的模块身份，不伪造路径。
fn expr_diagnostic_source(module: &HirModule, expr: &HirExpr, inherited: &DiagnosticSource) -> DiagnosticSource {
    match module.diagnostic_sources.get(&expr.span.source.version_id) {
        Some(path) => DiagnosticSource { id: path.clone(), revision: None },
        None => inherited.clone(),
    }
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
        HirError::UnresolvedCall { selector, context, reason, source, span } => DiagnosticRow::new(
            error.code(),
            DiagnosticSeverity::Error,
            DiagnosticPhase::Hir,
            DiagnosticOwner::frontend("rusty-valkyrie"),
            source.clone(),
            span.clone(),
        )
        .with_arg("selector", DiagnosticArg::String(selector.clone()))
        .with_arg("context", DiagnosticArg::String(context.clone()))
        .with_arg("reason", DiagnosticArg::String(reason.tag().to_string())),
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
        HirError::UnresolvedCall { selector, context, reason, .. } => format!("unresolved call `{selector}` in {context}: {}", reason.describe()),
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
            collect_function_call_contracts(hir, function, source, errors, false);
        }
    }
    for function in &hir.functions {
        collect_function_call_contracts(hir, function, source, errors, false);
    }
    for structure in &hir.structs {
        for function in &structure.methods {
            collect_function_call_contracts(hir, function, source, errors, false);
        }
        for property in &structure.properties {
            if let Some(function) = &property.getter {
                collect_function_call_contracts(hir, function, source, errors, false);
            }
            if let Some(function) = &property.setter {
                collect_function_call_contracts(hir, function, source, errors, false);
            }
        }
    }
    for trait_def in &hir.traits {
        for function in trait_def.methods.iter().chain(&trait_def.default_methods) {
            collect_function_call_contracts(hir, function, source, errors, false);
        }
    }
    for implementation in &hir.impls {
        for function in &implementation.methods {
            collect_function_call_contracts(hir, function, source, errors, false);
        }
    }
    // 单态化铸造的克隆体同样是会进入 MIR 的真实函数体：其签名与体内类型已完全代入，
    // 固定点按 where 证据拒绝具体代入时，克隆体内的调用点保持未闭合。声明体的开放
    // 占位仍由后续具体克隆闭合，不能在这里提前拒绝，因此只有克隆体要求实例已闭合。
    for callable in &hir.instantiated_callables {
        collect_function_call_contracts(hir, &callable.function, source, errors, true);
    }
}

// 仅保留给 `lowering::mod` 的 pub(crate) 门面 re-export；收集式遍历不再经此入口。
#[allow(dead_code)]
pub(crate) fn validate_function_call_contracts(function: &HirFunction, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_function_call_contracts(&HirModule::default(), function, source, &mut errors, false);
    errors.into_iter().next().map_or(Ok(()), Err)
}

fn collect_function_call_contracts(
    module: &HirModule,
    function: &HirFunction,
    source: &DiagnosticSource,
    errors: &mut Vec<HirError>,
    require_closed_instance: bool,
) {
    collect_block_call_contracts(module, &function.body, &function.name.to_string(), source, errors, require_closed_instance);
}

// 仅保留给 `lowering::mod` 的 pub(crate) 门面 re-export；收集式遍历不再经此入口。
#[allow(dead_code)]
pub(crate) fn validate_block_call_contracts(block: &HirBlock, function: &str, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_block_call_contracts(&HirModule::default(), block, function, source, &mut errors, false);
    errors.into_iter().next().map_or(Ok(()), Err)
}

fn collect_block_call_contracts(
    module: &HirModule,
    block: &HirBlock,
    function: &str,
    source: &DiagnosticSource,
    errors: &mut Vec<HirError>,
    require_closed_instance: bool,
) {
    for statement in &block.statements {
        match &statement.kind {
            HirStatementKind::Let { initializer, .. } => {
                if let Some(initializer) = initializer {
                    collect_expr_call_contracts(module, initializer, function, source, errors, require_closed_instance);
                }
            }
            HirStatementKind::Expr(expr) => collect_expr_call_contracts(module, expr, function, source, errors, require_closed_instance),
        }
    }
    if let Some(expr) = &block.expr {
        collect_expr_call_contracts(module, expr, function, source, errors, require_closed_instance);
    }
}

#[doc(hidden)]
pub fn validate_expr_call_contracts(expr: &HirExpr, function: &str, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut errors = Vec::new();
    collect_expr_call_contracts(&HirModule::default(), expr, function, source, &mut errors, false);
    errors.into_iter().next().map_or(Ok(()), Err)
}

/// 调用合同是否尚未闭合。
///
/// 除 `resolved` 缺失外，已单态化克隆体内以 `where` 证据拒绝具体代入的调用点会保留
/// 声明身份却清空实例：这类调用既没有实例身份也没有 trait witness，后端无法按名字
/// 或上下文恢复身份，必须与未解析调用一样在 HIR 边界结构化失败。声明体中的开放占位
/// 仍会由后续具体克隆闭合，故只在克隆体上要求实例已闭合；构造调用走独立的构造合同
/// 路径，不在此列。
fn resolved_call_is_unclosed(resolved: Option<&HirResolvedCall>, require_closed_instance: bool) -> bool {
    match resolved {
        None => true,
        Some(call) => {
            // 只有实参/返回类型都已具体化的调用才可能真正「缺实例」：仍携带开放 binder
            // 的调用是声明体内的自递归/跨结构占位，会由后续具体克隆闭合，不能提前拒绝。
            require_closed_instance
                && call.instance.is_none()
                && call.trait_dispatch.is_none()
                && call.declaration.is_some()
                && !matches!(call.domain, HirCallableDomain::Constructor)
                && !has_unresolved_generic_arguments(&call.return_type)
                && call.parameter_types.iter().all(|parameter| !has_unresolved_generic_arguments(parameter))
        }
    }
}

/// 前序遍历表达式树，收集全部未解析调用/构造；顺序与旧的首次失败遍历一致，
/// 因此收集版返回的首个错误与旧行为逐字节相同。
fn collect_expr_call_contracts(
    module: &HirModule,
    expr: &HirExpr,
    function: &str,
    source: &DiagnosticSource,
    errors: &mut Vec<HirError>,
    require_closed_instance: bool,
) {
    match &expr.kind {
        HirExprKind::Call { callee, args, resolved, .. } => {
            if resolved_call_is_unclosed(resolved.as_ref(), require_closed_instance) {
                let callee_name = callee.kind.callable_label();
                // 原因只取解析期登记的事实；克隆体与门面路径按结构化兜底补记，
                // 任何分支都不得回退到占位符。
                let reason = module
                    .call_resolution_failures
                    .iter()
                    .find(|failure| {
                        failure.span.source.version_id == expr.span.source.version_id
                            && failure.span.get_start() == expr.span.get_start()
                            && failure.span.get_end() == expr.span.get_end()
                    })
                    .map(|failure| failure.reason.clone())
                    .unwrap_or_else(|| {
                        if resolved.is_some() {
                            crate::compiler::types::hir::HirCallResolutionFailureKind::InstanceNotClosed
                        }
                        else {
                            crate::compiler::types::hir::HirCallResolutionFailureKind::FailureUnrecorded
                        }
                    });
                errors.push(HirError::UnresolvedCall {
                    selector: callee_name,
                    context: function.to_string(),
                    reason,
                    source: expr_diagnostic_source(module, expr, source),
                    span: expr_diagnostic_span(expr),
                });
            }
            collect_expr_call_contracts(module, callee, function, source, errors, require_closed_instance);
            for arg in args {
                collect_expr_call_contracts(module, &arg.value, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::Construct { path, args, resolved, .. } => {
            if resolved.is_none() {
                errors.push(HirError::UnresolvedConstructor {
                    name: path.to_string(),
                    context: function.to_string(),
                    source: expr_diagnostic_source(module, expr, source),
                    span: expr_diagnostic_span(expr),
                });
            }
            for arg in args {
                collect_expr_call_contracts(module, arg, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::FieldInit { value, .. }
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::TryPropagate { inner: value, .. }
        | HirExprKind::Raise(value)
        | HirExprKind::Resume(value) => collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance),
        HirExprKind::Cast { expr, .. } => collect_expr_call_contracts(module, expr, function, source, errors, require_closed_instance),
        HirExprKind::ArrayNew { length, .. } => collect_expr_call_contracts(module, length, function, source, errors, require_closed_instance),
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } | HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                collect_expr_call_contracts(module, item, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::FieldAccess { object, .. } => collect_expr_call_contracts(module, object, function, source, errors, require_closed_instance),
        HirExprKind::StoreField { object, value, .. } => {
            collect_expr_call_contracts(module, object, function, source, errors, require_closed_instance);
            collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance);
        }
        HirExprKind::GenericApply { callee, .. } => collect_expr_call_contracts(module, callee, function, source, errors, require_closed_instance),
        HirExprKind::Block(block) => collect_block_call_contracts(module, block, function, source, errors, require_closed_instance),
        HirExprKind::Lambda { body, .. } => collect_block_call_contracts(module, body, function, source, errors, require_closed_instance),
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance);
            }
            for method in methods {
                collect_function_call_contracts(module, method, source, errors, require_closed_instance);
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            collect_expr_call_contracts(module, condition, function, source, errors, require_closed_instance);
            collect_block_call_contracts(module, then_branch, function, source, errors, require_closed_instance);
            if let Some(block) = else_branch {
                collect_block_call_contracts(module, block, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            collect_expr_call_contracts(module, scrutinee, function, source, errors, require_closed_instance);
            collect_block_call_contracts(module, then_branch, function, source, errors, require_closed_instance);
            if let Some(block) = else_branch {
                collect_block_call_contracts(module, block, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } | HirExprKind::Catch { expr: scrutinee, arms } => {
            collect_expr_call_contracts(module, scrutinee, function, source, errors, require_closed_instance);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_call_contracts(module, guard, function, source, errors, require_closed_instance);
                }
                collect_expr_call_contracts(module, &arm.body, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                collect_expr_call_contracts(module, iterator, function, source, errors, require_closed_instance);
            }
            if let Some(condition) = condition {
                collect_expr_call_contracts(module, condition, function, source, errors, require_closed_instance);
            }
            collect_block_call_contracts(module, body, function, source, errors, require_closed_instance);
        }
        HirExprKind::Return(value) | HirExprKind::Yield(value) => {
            if let Some(value) = value {
                collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::Assign { value, .. } => collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance),
        HirExprKind::Break { expr, .. } => {
            if let Some(value) = expr {
                collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::TryScope { body, .. } => collect_block_call_contracts(module, body, function, source, errors, require_closed_instance),
        HirExprKind::With { base, updates } => {
            collect_expr_call_contracts(module, base, function, source, errors, require_closed_instance);
            for (_, value) in updates {
                collect_expr_call_contracts(module, value, function, source, errors, require_closed_instance);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                collect_expr_call_contracts(module, arg, function, source, errors, require_closed_instance);
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
