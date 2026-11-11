//! 字符串插值的 post-lowering 脱糖 pass（§4.10 E / §4.12 前端闭合门）。
//!
//! 带插值的引号捕获在调用解析之前改写成普通调用树：
//!
//! - 文本段冻结为 `text_encoding: Some(Utf8)` 的文本字面量；
//! - 插值段改写为接收者 `display` 成员调用，由既有 `resolve_hir_calls` 按接收者类型
//!   绑定 `std.text::Display::display`，本 pass 不写候选匹配或类型名白名单；
//! - 段与段之间用普通 `infix +` 调用左结合连接，单段直接返回该段。
//!
//! 改写保持源码顺序，每个插值表达式恰好出现一次；`is_fluent` 没有冻结的 locale
//! 合同，带源区间结构化失败，不按 Display 静默处理。解析失败（如类型没有 Display
//! 实现）留给既有调用解析与合同校验带源区间报错，本 pass 不回退占位文本。

use crate::compiler::{
    hir::lowering::{errors::HirError, quote_literal::settle_quote_error},
    types::{
        Identifier, NamePath, SourceSpan,
        hir::{
            HirBlock, HirCallArgument, HirExpr, HirExprKind, HirFunction, HirLiteral, HirModule, HirStatementKind, HirStringLiteral,
            HirStringSegment, HirTextEncoding,
        },
    },
};

/// 把模块（含子模块）内全部带插值的引号捕获脱糖为普通调用树。
///
/// 必须与 `collection_init::transform_collection_literals` 在同一阶段运行：调用解析
/// 之前，这样新增的 `display` 与 `infix +` 调用能被正常冻结。
pub(crate) fn desugar_string_interpolations(module: &mut HirModule) -> Result<(), HirError> {
    transform_module(module)
}

fn transform_module(module: &mut HirModule) -> Result<(), HirError> {
    for function in &mut module.functions {
        transform_function(function)?;
    }
    for structure in &mut module.structs {
        for method in &mut structure.methods {
            transform_function(method)?;
        }
    }
    for singleton in &mut module.singletons {
        for method in singleton
            .methods
            .iter_mut()
            .chain(singleton.constructor.iter_mut().map(Box::as_mut))
            .chain(singleton.finalizer.iter_mut().map(Box::as_mut))
        {
            transform_function(method)?;
        }
    }
    for trait_definition in &mut module.traits {
        for method in trait_definition.methods.iter_mut().chain(trait_definition.default_methods.iter_mut()) {
            transform_function(method)?;
        }
    }
    for implementation in &mut module.impls {
        for method in &mut implementation.methods {
            transform_function(method)?;
        }
    }
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            transform_function(method)?;
        }
    }
    for submodule in &mut module.submodules {
        transform_module(submodule)?;
    }
    Ok(())
}

fn transform_function(function: &mut HirFunction) -> Result<(), HirError> {
    transform_block(&mut function.body)
}

fn transform_block(block: &mut HirBlock) -> Result<(), HirError> {
    for statement in &mut block.statements {
        match &mut statement.kind {
            HirStatementKind::Let { initializer, .. } => {
                if let Some(initializer) = initializer.as_mut() {
                    transform_expr(initializer)?;
                }
            }
            HirStatementKind::Expr(expr) => transform_expr(expr)?,
        }
    }
    if let Some(expr) = &mut block.expr {
        transform_expr(expr)?;
    }
    Ok(())
}

/// 表达式复合结构下钻；分支面与 `collection_init::transform_expr` 镜像。
fn transform_expr(expr: &mut HirExpr) -> Result<(), HirError> {
    // 先递归处理插值表达式自身，保证嵌套字符串也被脱糖，再改写外层捕获。
    if let HirExprKind::Literal(HirLiteral::String(capture)) = &mut expr.kind {
        if capture.is_plain_text() {
            return Ok(());
        }
        for segment in &mut capture.segments {
            if let HirStringSegment::Interpolation { expr, .. } = segment {
                transform_expr(expr)?;
            }
        }
    }
    let replaced = std::mem::replace(&mut expr.kind, HirExprKind::Literal(HirLiteral::Unit));
    match replaced {
        HirExprKind::Literal(HirLiteral::String(capture)) if !capture.is_plain_text() => {
            expr.kind = desugar_interpolated_capture(capture, &expr.span)?;
        }
        HirExprKind::Call { callee, args, resolved } => {
            let mut callee = callee;
            transform_expr(&mut callee)?;
            let mut lowered = Vec::with_capacity(args.len());
            for arg in args {
                let mut arg = arg;
                transform_expr(&mut arg.value)?;
                lowered.push(arg);
            }
            expr.kind = HirExprKind::Call { callee, args: lowered, resolved };
        }
        HirExprKind::Construct { path, generic_arguments, args, resolved } => {
            let mut lowered = Vec::with_capacity(args.len());
            for arg in args {
                let mut arg = arg;
                transform_expr(&mut arg)?;
                lowered.push(arg);
            }
            expr.kind = HirExprKind::Construct { path, generic_arguments, args: lowered, resolved };
        }
        HirExprKind::FieldInit { name, value, field_id } => {
            let mut value = value;
            transform_expr(&mut value)?;
            expr.kind = HirExprKind::FieldInit { name, value, field_id };
        }
        HirExprKind::FieldAccess { object, field, field_id, tuple_index } => {
            let mut object = object;
            transform_expr(&mut object)?;
            expr.kind = HirExprKind::FieldAccess { object, field, field_id, tuple_index };
        }
        HirExprKind::StoreField { object, field, value, field_id } => {
            let mut object = object;
            let mut value = value;
            transform_expr(&mut object)?;
            transform_expr(&mut value)?;
            expr.kind = HirExprKind::StoreField { object, field, value, field_id };
        }
        HirExprKind::Assign { target, value } => {
            let mut value = value;
            transform_expr(&mut value)?;
            expr.kind = HirExprKind::Assign { target, value };
        }
        HirExprKind::GenericApply { callee, arguments } => {
            let mut callee = callee;
            transform_expr(&mut callee)?;
            expr.kind = HirExprKind::GenericApply { callee, arguments };
        }
        HirExprKind::ArrayNew { element_type, length } => {
            let mut length = length;
            transform_expr(&mut length)?;
            expr.kind = HirExprKind::ArrayNew { element_type, length };
        }
        HirExprKind::ArrayLiteral { items } => {
            let mut lowered = Vec::with_capacity(items.len());
            for mut item in items {
                transform_expr(&mut item)?;
                lowered.push(item);
            }
            expr.kind = HirExprKind::ArrayLiteral { items: lowered };
        }
        HirExprKind::TupleLiteral { items } => {
            let mut lowered = Vec::with_capacity(items.len());
            for mut item in items {
                transform_expr(&mut item)?;
                lowered.push(item);
            }
            expr.kind = HirExprKind::TupleLiteral { items: lowered };
        }
        HirExprKind::CollectionInit { container_type, element_type, with_length, append, items } => {
            let mut lowered = Vec::with_capacity(items.len());
            for mut item in items {
                transform_expr(&mut item)?;
                lowered.push(item);
            }
            expr.kind = HirExprKind::CollectionInit { container_type, element_type, with_length, append, items: lowered };
        }
        HirExprKind::Block(block) => {
            let mut block = block;
            transform_block(&mut block)?;
            expr.kind = HirExprKind::Block(block);
        }
        HirExprKind::TryScope { is_optional, is_forced, result_type, body } => {
            let mut body = body;
            transform_block(&mut body)?;
            expr.kind = HirExprKind::TryScope { is_optional, is_forced, result_type, body };
        }
        HirExprKind::Lambda { generics, params, return_type, body, captures, hoisted } => {
            let mut body = body;
            transform_block(&mut body)?;
            expr.kind = HirExprKind::Lambda { generics, params, return_type, body, captures, hoisted };
        }
        HirExprKind::AnonymousClass { is_value_type, parents, fields, methods, captures, class_name } => {
            let mut lowered_fields = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                let mut value = value;
                transform_expr(&mut value)?;
                lowered_fields.push((name, value));
            }
            let mut methods = methods;
            for method in &mut methods {
                transform_function(method)?;
            }
            expr.kind = HirExprKind::AnonymousClass { is_value_type, parents, fields: lowered_fields, methods, captures, class_name };
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            let mut condition = condition;
            let mut then_branch = then_branch;
            let mut else_branch = else_branch;
            transform_expr(&mut condition)?;
            transform_block(&mut then_branch)?;
            if let Some(branch) = else_branch.as_mut() {
                transform_block(branch)?;
            }
            expr.kind = HirExprKind::If { condition, then_branch, else_branch };
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            let mut scrutinee = scrutinee;
            let mut then_branch = then_branch;
            let mut else_branch = else_branch;
            transform_expr(&mut scrutinee)?;
            transform_block(&mut then_branch)?;
            if let Some(branch) = else_branch.as_mut() {
                transform_block(branch)?;
            }
            expr.kind = HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch };
        }
        HirExprKind::Match { scrutinee, arms } => {
            let mut scrutinee = scrutinee;
            transform_expr(&mut scrutinee)?;
            let mut arms = arms;
            transform_arms(&mut arms)?;
            expr.kind = HirExprKind::Match { scrutinee, arms };
        }
        HirExprKind::Case { scrutinee, arms } => {
            let mut scrutinee = scrutinee;
            transform_expr(&mut scrutinee)?;
            let mut arms = arms;
            transform_arms(&mut arms)?;
            expr.kind = HirExprKind::Case { scrutinee, arms };
        }
        HirExprKind::Loop { label, pattern, iterator, condition, body } => {
            let mut iterator = iterator;
            let mut condition = condition;
            let mut body = body;
            if let Some(iterator) = iterator.as_mut() {
                transform_expr(iterator)?;
            }
            if let Some(condition) = condition.as_mut() {
                transform_expr(condition)?;
            }
            transform_block(&mut body)?;
            expr.kind = HirExprKind::Loop { label, pattern, iterator, condition, body };
        }
        HirExprKind::Catch { expr: inner, arms } => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            let mut arms = arms;
            transform_arms(&mut arms)?;
            expr.kind = HirExprKind::Catch { expr: inner, arms };
        }
        HirExprKind::With { base, updates } => {
            let mut base = base;
            transform_expr(&mut base)?;
            let mut lowered = Vec::with_capacity(updates.len());
            for (name, value) in updates {
                let mut value = value;
                transform_expr(&mut value)?;
                lowered.push((name, value));
            }
            expr.kind = HirExprKind::With { base, updates: lowered };
        }
        HirExprKind::SuperCall { parent_alias, method, args } => {
            let mut lowered = Vec::with_capacity(args.len());
            for mut arg in args {
                transform_expr(&mut arg)?;
                lowered.push(arg);
            }
            expr.kind = HirExprKind::SuperCall { parent_alias, method, args: lowered };
        }
        HirExprKind::Return(value) => {
            let mut value = value;
            if let Some(inner) = value.as_mut() {
                transform_expr(inner)?;
            }
            expr.kind = HirExprKind::Return(value);
        }
        HirExprKind::Break { label, expr: value } => {
            let mut value = value;
            if let Some(inner) = value.as_mut() {
                transform_expr(inner)?;
            }
            expr.kind = HirExprKind::Break { label, expr: value };
        }
        HirExprKind::Yield(value) => {
            let mut value = value;
            if let Some(inner) = value.as_mut() {
                transform_expr(inner)?;
            }
            expr.kind = HirExprKind::Yield(value);
        }
        HirExprKind::Cast { expr: inner, ty } => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::Cast { expr: inner, ty };
        }
        HirExprKind::TryPropagate { inner, resolved_ok_variant } => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::TryPropagate { inner, resolved_ok_variant };
        }
        HirExprKind::YieldFrom(inner) => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::YieldFrom(inner);
        }
        HirExprKind::Await(inner) => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::Await(inner);
        }
        HirExprKind::Awake(inner) => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::Awake(inner);
        }
        HirExprKind::BlockOn(inner) => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::BlockOn(inner);
        }
        HirExprKind::Raise(inner) => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::Raise(inner);
        }
        HirExprKind::Resume(inner) => {
            let mut inner = inner;
            transform_expr(&mut inner)?;
            expr.kind = HirExprKind::Resume(inner);
        }
        other => expr.kind = other,
    }
    Ok(())
}

fn transform_arms(arms: &mut [crate::compiler::types::hir::HirMatchArm]) -> Result<(), HirError> {
    for arm in arms {
        if let Some(guard) = arm.guard.as_mut() {
            transform_expr(guard)?;
        }
        transform_expr(&mut arm.body)?;
    }
    Ok(())
}

/// 把一个带插值的捕获改写成普通调用树，保持从左到右、每个插值恰好出现一次。
fn desugar_interpolated_capture(capture: HirStringLiteral, span: &SourceSpan) -> Result<HirExprKind, HirError> {
    let quote = capture.quote;
    let quote_count = capture.quote_count;
    let mut segments: Vec<HirExpr> = Vec::new();
    for segment in capture.segments {
        match segment {
            HirStringSegment::Text(text) => {
                if text.is_empty() {
                    continue;
                }
                segments.push(HirExpr {
                    kind: HirExprKind::Literal(HirLiteral::String(HirStringLiteral {
                        prefix: None,
                        quote_count,
                        quote,
                        segments: vec![HirStringSegment::Text(text)],
                        text_encoding: Some(HirTextEncoding::Utf8),
                    })),
                    span: span.clone(),
                });
            }
            HirStringSegment::Interpolation { expr, is_fluent } => {
                if is_fluent {
                    return Err(settle_quote_error(
                        crate::compiler::hir::lowering::errors::QuoteLiteralFailure::TemplateSyntax {
                            message: "fluent interpolation requires a frozen locale contract".to_string(),
                        },
                        expr.span.span.start as usize..expr.span.span.end as usize,
                    ));
                }
                let expr_span = expr.span.clone();
                segments.push(HirExpr { kind: display_call_kind(expr, &expr_span), span: expr_span });
            }
        }
    }
    let mut segments = segments.into_iter();
    let Some(mut accumulated) = segments.next()
    else {
        return Ok(HirExprKind::Literal(HirLiteral::Unit));
    };
    for rhs in segments {
        accumulated = HirExpr { kind: concat_call_kind(accumulated, rhs, span), span: span.clone() };
    }
    Ok(accumulated.kind)
}

/// 插值段 → `<expr>.display()` 普通成员调用；实例由既有调用解析冻结。
fn display_call_kind(object: HirExpr, span: &SourceSpan) -> HirExprKind {
    HirExprKind::Call {
        callee: Box::new(HirExpr {
            kind: HirExprKind::FieldAccess {
                object: Box::new(object),
                field: Identifier::new(crate::compiler::semantic::operator_members::INTERPOLATION_DISPLAY_MEMBER),
                field_id: None,
                tuple_index: None,
            },
            span: span.clone(),
        }),
        args: Vec::new(),
        resolved: None,
    }
}

/// 段连接 → `infix +` 普通调用，形状与 `lower_operator_member_call` 一致。
fn concat_call_kind(lhs: HirExpr, rhs: HirExpr, span: &SourceSpan) -> HirExprKind {
    HirExprKind::Call {
        callee: Box::new(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new("infix +")])), span: span.clone() }),
        args: vec![HirCallArgument::positional(lhs), HirCallArgument::positional(rhs)],
        resolved: None,
    }
}
