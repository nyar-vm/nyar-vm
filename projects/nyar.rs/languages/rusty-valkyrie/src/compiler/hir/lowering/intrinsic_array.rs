//! `builtin.array.new` 的 post-binding 规范化 pass。
//!
//! 源码声明的 `[intrinsic("builtin.array.new")]` 注解在候选构造阶段冻结为
//! [`BuiltinStructureOp::ArrayNew`](crate::compiler::types::hir::BuiltinStructureOp::ArrayNew)，
//! 调用解析按声明正常签名完成（含显式泛型实参）。本 pass 在**调用解析之后、
//! 泛型单态化之前**把这类已解析 `Call` 规范化为 `HirExprKind::ArrayNew`：
//!
//! - 元素类型只来自解析结果冻结的返回数组形状 `[T]`，返回不是数组形状即结构化拒绝；
//! - 长度实参必须恰好 1 个，否则结构化拒绝；
//! - 表达式 span 保持调用点原 span，不合成源码。
//!
//! 泛型 binder 由既有 `instantiate_generic_callables` 的替换路径（`substitute_expr`
//! 的 `ArrayNew` 分支）在实例体内具体化，本 pass 不在此处展开泛型。

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    hir::{control_flow_validation::display_type, lowering::errors::HirError},
    types::{
        SourceSpan,
        hir::{BuiltinStructureOp, HirBlock, HirExpr, HirExprKind, HirFunction, HirModule, HirStatementKind, ValkyrieType},
    },
};

fn diagnostic_span(span: &SourceSpan) -> DiagnosticSpan {
    DiagnosticSpan { start: span.get_start() as usize, end: span.get_end() as usize }
}

/// 把已解析的 `builtin.array.new` 调用替换成 `HirExprKind::ArrayNew`。
fn convert_call(expr: &mut HirExpr, source: &DiagnosticSource) -> Result<(), HirError> {
    let span = diagnostic_span(&expr.span);
    let (element_type, length) = {
        let HirExprKind::Call { args, resolved, .. } = &mut expr.kind
        else {
            return Ok(());
        };
        let Some(resolved) = resolved.as_ref()
        else {
            return Ok(());
        };
        if resolved.intrinsic != Some(BuiltinStructureOp::ArrayNew) {
            return Ok(());
        }
        if args.len() != 1 {
            return Err(HirError::ArrayNewArityMismatch { expected: 1, actual: args.len(), source: source.clone(), span });
        }
        let element_type = match &resolved.return_type {
            ValkyrieType::Array(element) => element.as_ref().clone(),
            other => {
                return Err(HirError::ArrayNewReturnTypeNotArray { actual: display_type(other), source: source.clone(), span });
            }
        };
        let length = args.pop().expect("array new arity was checked").value;
        (element_type, length)
    };
    expr.kind = HirExprKind::ArrayNew { element_type, length: Box::new(length) };
    Ok(())
}

/// 表达式下钻；分支面与单态化固定点 `visit_expr` 镜像，覆盖全部承载子表达式的节点。
fn normalize_expr(expr: &mut HirExpr, source: &DiagnosticSource) -> Result<(), HirError> {
    let mut convert = false;
    match &mut expr.kind {
        HirExprKind::Call { callee, args, .. } => {
            normalize_expr(callee, source)?;
            for arg in args {
                normalize_expr(&mut arg.value, source)?;
            }
            convert = true;
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                normalize_expr(arg, source)?;
            }
        }
        HirExprKind::Cast { expr: inner, .. }
        | HirExprKind::FieldAccess { object: inner, .. }
        | HirExprKind::FieldInit { value: inner, .. }
        | HirExprKind::Await(inner)
        | HirExprKind::Awake(inner)
        | HirExprKind::BlockOn(inner)
        | HirExprKind::YieldFrom(inner)
        | HirExprKind::Raise(inner)
        | HirExprKind::Resume(inner)
        | HirExprKind::Assign { value: inner, .. }
        | HirExprKind::Return(Some(inner))
        | HirExprKind::Yield(Some(inner))
        | HirExprKind::Break { expr: Some(inner), .. } => normalize_expr(inner, source)?,
        HirExprKind::TryPropagate { inner, .. } => normalize_expr(inner, source)?,
        HirExprKind::StoreField { object, value, .. } => {
            normalize_expr(object, source)?;
            normalize_expr(value, source)?;
        }
        HirExprKind::GenericApply { callee, .. } => normalize_expr(callee, source)?,
        HirExprKind::ArrayNew { length, .. } => normalize_expr(length, source)?,
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
            for item in items {
                normalize_expr(item, source)?;
            }
        }
        HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                normalize_expr(item, source)?;
            }
        }
        HirExprKind::Block(block) | HirExprKind::TryScope { body: block, .. } => normalize_block(block, source)?,
        HirExprKind::Lambda { body, .. } => normalize_block(body, source)?,
        HirExprKind::If { condition, then_branch, else_branch } => {
            normalize_expr(condition, source)?;
            normalize_block(then_branch, source)?;
            if let Some(branch) = else_branch {
                normalize_block(branch, source)?;
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            normalize_expr(scrutinee, source)?;
            normalize_block(then_branch, source)?;
            if let Some(branch) = else_branch {
                normalize_block(branch, source)?;
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            normalize_expr(scrutinee, source)?;
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    normalize_expr(guard, source)?;
                }
                normalize_expr(&mut arm.body, source)?;
            }
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                normalize_expr(iterator, source)?;
            }
            if let Some(condition) = condition {
                normalize_expr(condition, source)?;
            }
            normalize_block(body, source)?;
        }
        HirExprKind::Catch { expr: inner, arms } => {
            normalize_expr(inner, source)?;
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    normalize_expr(guard, source)?;
                }
                normalize_expr(&mut arm.body, source)?;
            }
        }
        HirExprKind::With { base, updates } => {
            normalize_expr(base, source)?;
            for (_, update) in updates {
                normalize_expr(update, source)?;
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                normalize_expr(arg, source)?;
            }
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                normalize_expr(value, source)?;
            }
            for method in methods {
                normalize_function(method, source)?;
            }
        }
        _ => {}
    }
    if convert {
        convert_call(expr, source)?;
    }
    Ok(())
}

fn normalize_block(block: &mut HirBlock, source: &DiagnosticSource) -> Result<(), HirError> {
    for statement in &mut block.statements {
        match &mut statement.kind {
            HirStatementKind::Let { initializer: Some(initializer), .. } => normalize_expr(initializer, source)?,
            HirStatementKind::Expr(expr) => normalize_expr(expr, source)?,
            HirStatementKind::Let { initializer: None, .. } => {}
        }
    }
    if let Some(expr) = &mut block.expr {
        normalize_expr(expr, source)?;
    }
    Ok(())
}

fn normalize_function(function: &mut HirFunction, source: &DiagnosticSource) -> Result<(), HirError> {
    normalize_block(&mut function.body, source)
}

fn normalize_module(module: &mut HirModule, source: &DiagnosticSource) -> Result<(), HirError> {
    for function in &mut module.functions {
        normalize_function(function, source)?;
    }
    for structure in &mut module.structs {
        for method in &mut structure.methods {
            normalize_function(method, source)?;
        }
        for property in &mut structure.properties {
            for accessor in property.getter.iter_mut().chain(property.setter.iter_mut()) {
                normalize_function(accessor, source)?;
            }
        }
    }
    for singleton in &mut module.singletons {
        for method in singleton
            .methods
            .iter_mut()
            .chain(singleton.constructor.iter_mut().map(Box::as_mut))
            .chain(singleton.finalizer.iter_mut().map(Box::as_mut))
        {
            normalize_function(method, source)?;
        }
    }
    for trait_definition in &mut module.traits {
        for method in trait_definition.methods.iter_mut().chain(trait_definition.default_methods.iter_mut()) {
            normalize_function(method, source)?;
        }
    }
    for implementation in &mut module.impls {
        for method in &mut implementation.methods {
            normalize_function(method, source)?;
        }
    }
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            normalize_function(method, source)?;
        }
    }
    for submodule in &mut module.submodules {
        normalize_module(submodule, source)?;
    }
    Ok(())
}

/// 规范化入口：调用解析之后、泛型单态化之前运行。
pub(crate) fn normalize_intrinsic_array_new(module: &mut HirModule) -> Result<(), HirError> {
    let source = if module.source_path.trim().is_empty() {
        DiagnosticSource { id: "<hir_module>".into(), revision: None }
    }
    else {
        DiagnosticSource { id: module.source_path.clone(), revision: None }
    };
    normalize_module(module, &source)
}
