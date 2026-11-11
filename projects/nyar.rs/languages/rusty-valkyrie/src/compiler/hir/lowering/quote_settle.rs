//! HIR 引号捕获唯一定型结算（§4.10 B / 01 §4.12 前端闭合门）。
//!
//! 调用解析冻结形参合同之后、单态化与 MIR 之前，把全部未定型引号捕获结算为最终
//! 字面量形态：注解、形参、返回、字段、容器元素与泛型约束先行联合求解；引号默认
//! 只用于确无约束的位置且只用一次。char 目标落 `HirLiteral::Character(u32)`，
//! 文本目标冻结 `text_encoding`；无约束 `''`/`'ab'` 默认 char 后报带源区间的表示
//! 错误，不提供长度回退。结算完成后 HIR 可独立证明每个字面量的最终类型，
//! MIR 只编码既定值，不再读引号、不做语言类型选择。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::compiler::{
    hir::{
        lowering::{
            errors::{HirError, QuoteLiteralFailure},
            quote_literal::{
                capture_binds, capture_code_points, capture_quote_default, settle_quote_error, CaptureTargetKind, SettledCapture,
            },
        },
        overload::types::{infer_scrutinee_type, lookup_struct_field_type},
        type_relation::TypeRelationContext,
    },
    mir::{AggregateFieldIndex, collect_aggregate_field_map},
    types::{Identifier, hir::*},
};

/// 对模块内全部未定型引号捕获执行唯一定型结算。
pub(crate) fn settle_quote_literal_captures(module: &mut HirModule) -> Result<(), HirError> {
    let relations = TypeRelationContext::from_module(module);
    let struct_fields = collect_aggregate_field_map(module);
    let singleton_names = module.singletons.iter().map(|singleton| singleton.name.clone()).collect::<BTreeSet<_>>();
    let pass = SettlePass { relations: &relations, struct_fields: &struct_fields, singleton_names: &singleton_names };
    settle_submodule(module, &pass)
}

/// 结算只消费已冻结的声明身份表与形参合同，不重新解析调用。
struct SettlePass<'a> {
    relations: &'a TypeRelationContext,
    struct_fields: &'a AggregateFieldIndex,
    singleton_names: &'a BTreeSet<Identifier>,
}

fn settle_submodule(module: &mut HirModule, pass: &SettlePass) -> Result<(), HirError> {
    for function in &mut module.functions {
        settle_function(function, pass)?;
    }
    for item in &mut module.structs {
        for method in &mut item.methods {
            settle_function(method, pass)?;
        }
    }
    for item in &mut module.singletons {
        for method in &mut item.methods {
            settle_function(method, pass)?;
        }
        if let Some(constructor) = &mut item.constructor {
            settle_function(constructor, pass)?;
        }
        if let Some(finalizer) = &mut item.finalizer {
            settle_function(finalizer, pass)?;
        }
    }
    for item in &mut module.traits {
        for method in item.methods.iter_mut().chain(item.default_methods.iter_mut()) {
            settle_function(method, pass)?;
        }
    }
    for item in &mut module.impls {
        for method in &mut item.methods {
            settle_function(method, pass)?;
        }
    }
    for submodule in &mut module.submodules {
        settle_submodule(submodule, pass)?;
    }
    Ok(())
}

fn settle_function(function: &mut HirFunction, pass: &SettlePass) -> Result<(), HirError> {
    let mut locals = function.params.iter().map(|param| (param.name.name.to_string(), param.ty.clone())).collect::<BTreeMap<_, _>>();
    let return_type = return_slot(&function.return_type);
    settle_block(&mut function.body, pass, &mut locals, return_type.clone(), return_type)
}

/// 返回槽期望：`AutoType`/`Void` 不构成文本或字符约束。
fn return_slot(ty: &ValkyrieType) -> Option<ValkyrieType> {
    (!matches!(ty, ValkyrieType::AutoType | ValkyrieType::Void)).then(|| ty.clone())
}

fn settle_block(
    block: &mut HirBlock,
    pass: &SettlePass,
    locals: &mut BTreeMap<String, ValkyrieType>,
    expected: Option<ValkyrieType>,
    return_type: Option<ValkyrieType>,
) -> Result<(), HirError> {
    for statement in &mut block.statements {
        settle_statement(statement, pass, locals, expected.clone(), return_type.clone())?;
    }
    if let Some(expr) = &mut block.expr {
        settle_expr(expr, pass, locals, expected, return_type)?;
    }
    Ok(())
}

fn settle_statement(
    statement: &mut HirStatement,
    pass: &SettlePass,
    locals: &mut BTreeMap<String, ValkyrieType>,
    expected: Option<ValkyrieType>,
    return_type: Option<ValkyrieType>,
) -> Result<(), HirError> {
    match &mut statement.kind {
        HirStatementKind::Let { pattern, initializer, ty, .. } => {
            if let Some(initializer) = initializer {
                // 显式注解才是权威期望类型；无注解 `let` 的初始值确无约束，按引号默认结算。
                settle_expr(initializer, pass, locals, ty.clone(), return_type)?;
                let binding_type =
                    ty.clone().or_else(|| infer_scrutinee_type(initializer, locals, pass.struct_fields, pass.singleton_names));
                if let Some(binding_type) = binding_type {
                    bind_pattern_name(pattern, &binding_type, locals);
                }
            }
        }
        HirStatementKind::Expr(expr) => settle_expr(expr, pass, locals, expected, return_type)?,
    }
    Ok(())
}

/// 记录 `let` 绑定名与其最终类型，供后续赋值位置取期望。
fn bind_pattern_name(pattern: &HirPattern, binding_type: &ValkyrieType, locals: &mut BTreeMap<String, ValkyrieType>) {
    match pattern {
        HirPattern::Variable(identifier) => {
            locals.insert(identifier.name.to_string(), binding_type.clone());
        }
        HirPattern::Bind { identifier, pattern } => {
            locals.insert(identifier.name.to_string(), binding_type.clone());
            bind_pattern_name(pattern, binding_type, locals);
        }
        HirPattern::Mut(inner) | HirPattern::Pin { pattern: inner, .. } => bind_pattern_name(inner, binding_type, locals),
        _ => {}
    }
}

fn settle_expr(
    expr: &mut HirExpr,
    pass: &SettlePass,
    locals: &mut BTreeMap<String, ValkyrieType>,
    expected: Option<ValkyrieType>,
    return_type: Option<ValkyrieType>,
) -> Result<(), HirError> {
    let span = expr.span.span.start as usize..expr.span.span.end as usize;
    match &mut expr.kind {
        HirExprKind::Literal(literal) => {
            if let HirLiteral::String(capture) = literal {
                // 插值段是独立表达式，无外层文本之外的约束；先结算内层再结算捕获本体。
                for segment in &mut capture.segments {
                    if let HirStringSegment::Interpolation { expr, .. } = segment {
                        settle_expr(expr, pass, locals, None, return_type.clone())?;
                    }
                }
                match settle_capture(capture, expected.as_ref(), pass, span)? {
                    Some(SettledCapture::Char(code)) => *literal = HirLiteral::Character(code),
                    Some(SettledCapture::Text(encoding)) => capture.text_encoding = Some(encoding),
                    None => {}
                }
            }
        }
        HirExprKind::Call { args, resolved, .. } => {
            let offset = usize::from(resolved.as_ref().is_some_and(|call| call.has_receiver));
            for (index, arg) in args.iter_mut().enumerate() {
                let formal = resolved.as_ref().and_then(|call| call.parameter_types.get(index + offset).cloned());
                settle_expr(&mut arg.value, pass, locals, formal, return_type.clone())?;
            }
        }
        HirExprKind::Construct { args, resolved, .. } => {
            for (index, arg) in args.iter_mut().enumerate() {
                let formal = resolved.as_ref().and_then(|call| call.parameter_types.get(index).cloned());
                settle_expr(arg, pass, locals, formal, return_type.clone())?;
            }
        }
        HirExprKind::FieldInit { value, .. } => settle_expr(value, pass, locals, expected, return_type)?,
        HirExprKind::ArrayLiteral { items } => {
            let element = expected.as_ref().and_then(element_type_of);
            for item in items.iter_mut() {
                settle_expr(item, pass, locals, element.clone(), return_type.clone())?;
            }
        }
        HirExprKind::TupleLiteral { items } => {
            let slots = expected.as_ref().and_then(slot_types_of);
            for (index, item) in items.iter_mut().enumerate() {
                let item_expected = slots.as_ref().and_then(|slots| slots.get(index).cloned());
                settle_expr(item, pass, locals, item_expected, return_type.clone())?;
            }
        }
        HirExprKind::CollectionInit { items, element_type, .. } => {
            for item in items.iter_mut() {
                settle_expr(item, pass, locals, Some(element_type.clone()), return_type.clone())?;
            }
        }
        HirExprKind::ArrayNew { length, .. } => settle_expr(length, pass, locals, None, return_type)?,
        HirExprKind::If { condition, then_branch, else_branch } => {
            settle_expr(condition, pass, locals, None, return_type.clone())?;
            settle_block(then_branch, pass, locals, expected.clone(), return_type.clone())?;
            if let Some(branch) = else_branch {
                settle_block(branch, pass, locals, expected, return_type)?;
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            settle_expr(scrutinee, pass, locals, None, return_type.clone())?;
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, pass.struct_fields, pass.singleton_names);
            settle_pattern(pattern, scrutinee_type, span.clone(), pass, locals)?;
            settle_block(then_branch, pass, locals, expected.clone(), return_type.clone())?;
            if let Some(branch) = else_branch {
                settle_block(branch, pass, locals, expected, return_type)?;
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            settle_expr(scrutinee, pass, locals, None, return_type.clone())?;
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, pass.struct_fields, pass.singleton_names);
            for arm in arms.iter_mut() {
                settle_pattern(&mut arm.pattern, scrutinee_type.clone(), span.clone(), pass, locals)?;
                if let Some(guard) = &mut arm.guard {
                    settle_expr(guard, pass, locals, None, return_type.clone())?;
                }
                settle_expr(&mut arm.body, pass, locals, expected.clone(), return_type.clone())?;
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                settle_expr(iterator, pass, locals, None, return_type.clone())?;
            }
            if let Some(pattern) = pattern {
                settle_pattern(pattern, None, span.clone(), pass, locals)?;
            }
            if let Some(condition) = condition {
                settle_expr(condition, pass, locals, None, return_type.clone())?;
            }
            settle_block(body, pass, locals, None, return_type)?;
        }
        HirExprKind::Return(value) => {
            if let Some(value) = value {
                settle_expr(value, pass, locals, return_type.clone(), return_type)?;
            }
        }
        HirExprKind::Assign { target, value } => {
            let target_type = locals.get(target.as_str()).cloned();
            settle_expr(value, pass, locals, target_type, return_type)?;
        }
        HirExprKind::StoreField { object, field, value, .. } => {
            settle_expr(object, pass, locals, None, return_type.clone())?;
            let object_type = infer_scrutinee_type(object, locals, pass.struct_fields, pass.singleton_names);
            let field_type = object_type.as_ref().and_then(|ty| lookup_struct_field_type(ty, field, pass.struct_fields));
            settle_expr(value, pass, locals, field_type, return_type)?;
        }
        HirExprKind::FieldAccess { object, .. } => settle_expr(object, pass, locals, None, return_type)?,
        HirExprKind::GenericApply { callee, .. } => settle_expr(callee, pass, locals, None, return_type)?,
        HirExprKind::Block(block) => settle_block(block, pass, locals, expected, return_type)?,
        HirExprKind::Lambda { params, return_type: lambda_return, body, .. } => {
            let mut inner_locals = locals.clone();
            for param in params {
                inner_locals.insert(param.name.name.to_string(), param.ty.clone());
            }
            let slot = return_slot(lambda_return);
            settle_block(body, pass, &mut inner_locals, slot.clone(), slot)?;
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields.iter_mut() {
                settle_expr(value, pass, locals, None, return_type.clone())?;
            }
            for method in methods.iter_mut() {
                settle_function(method, pass)?;
            }
        }
        HirExprKind::Yield(value) => {
            if let Some(value) = value {
                settle_expr(value, pass, locals, None, return_type.clone())?;
            }
        }
        HirExprKind::YieldFrom(value) | HirExprKind::Await(value) | HirExprKind::Awake(value) | HirExprKind::BlockOn(value) => {
            settle_expr(value, pass, locals, None, return_type)?;
        }
        HirExprKind::Raise(value) | HirExprKind::Resume(value) => settle_expr(value, pass, locals, None, return_type)?,
        HirExprKind::Catch { expr, arms } => {
            settle_expr(expr, pass, locals, None, return_type.clone())?;
            for arm in arms.iter_mut() {
                settle_pattern(&mut arm.pattern, None, span.clone(), pass, locals)?;
                if let Some(guard) = &mut arm.guard {
                    settle_expr(guard, pass, locals, None, return_type.clone())?;
                }
                settle_expr(&mut arm.body, pass, locals, expected.clone(), return_type.clone())?;
            }
        }
        HirExprKind::TryPropagate { inner, .. } => settle_expr(inner, pass, locals, None, return_type)?,
        HirExprKind::TryScope { body, .. } => settle_block(body, pass, locals, None, return_type)?,
        HirExprKind::Cast { expr, ty } => settle_expr(expr, pass, locals, Some(ty.clone()), return_type)?,
        HirExprKind::With { base, updates } => {
            settle_expr(base, pass, locals, None, return_type.clone())?;
            for (_, value) in updates.iter_mut() {
                settle_expr(value, pass, locals, None, return_type.clone())?;
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args.iter_mut() {
                settle_expr(arg, pass, locals, None, return_type.clone())?;
            }
        }
        HirExprKind::Break { expr, .. } => {
            if let Some(expr) = expr {
                settle_expr(expr, pass, locals, None, return_type)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// 数组/固定数组期望的元素类型。
fn element_type_of(ty: &ValkyrieType) -> Option<ValkyrieType> {
    match ty {
        ValkyrieType::Array(inner) => Some(inner.as_ref().clone()),
        ValkyrieType::FixedArray { element, .. } => Some(element.as_ref().clone()),
        _ => None,
    }
}

/// 元组/带标签元组期望的逐槽类型；其它期望无槽位信息。
fn slot_types_of(ty: &ValkyrieType) -> Option<Vec<ValkyrieType>> {
    match ty {
        ValkyrieType::Tuple(items) => Some(items.clone()),
        ValkyrieType::LabeledTuple(fields) => Some(fields.iter().map(|field| field.ty.clone()).collect()),
        _ => None,
    }
}

/// 结算模式位字面量：被检类型是权威约束，缺失时按引号默认。
fn settle_pattern(
    pattern: &mut HirPattern,
    expected: Option<ValkyrieType>,
    span: Range<usize>,
    pass: &SettlePass,
    locals: &mut BTreeMap<String, ValkyrieType>,
) -> Result<(), HirError> {
    match pattern {
        HirPattern::Literal(literal) => settle_pattern_literal(literal, expected.as_ref(), span, pass)?,
        HirPattern::Range { start, end, .. } => {
            for endpoint in start.iter_mut().chain(end.iter_mut()) {
                settle_pattern_literal(endpoint, expected.as_ref(), span.clone(), pass)?;
            }
        }
        HirPattern::Tuple(subpatterns) => {
            let slots = expected.as_ref().and_then(slot_types_of);
            for (index, subpattern) in subpatterns.iter_mut().enumerate() {
                let slot = slots.as_ref().and_then(|slots| slots.get(index).cloned());
                settle_pattern(subpattern, slot, span.clone(), pass, locals)?;
            }
        }
        HirPattern::Or(subpatterns) => {
            for subpattern in subpatterns.iter_mut() {
                settle_pattern(subpattern, expected.clone(), span.clone(), pass, locals)?;
            }
        }
        HirPattern::Bind { identifier, pattern } => {
            if let Some(expected) = &expected {
                locals.insert(identifier.name.to_string(), expected.clone());
            }
            settle_pattern(pattern, expected, span, pass, locals)?;
        }
        HirPattern::Mut(inner) => settle_pattern(inner, expected, span, pass, locals)?,
        HirPattern::Pin { pattern: inner, .. } => settle_pattern(inner, expected, span, pass, locals)?,
        HirPattern::Object { fields, named_fields, .. } => {
            let slots = expected.as_ref().and_then(|ty| {
                pass.struct_fields
                    .fields_of(ty)
                    .map(|declared| declared.iter().map(|field| (field.name.clone(), field.ty.clone())).collect::<Vec<_>>())
            });
            for (index, (name, subpattern)) in fields.iter_mut().enumerate() {
                let field_type = if *named_fields {
                    slots
                        .as_ref()
                        .and_then(|slots| slots.iter().find(|(declared, _)| declared == name))
                        .map(|(_, ty)| ty.clone())
                }
                else {
                    slots.as_ref().and_then(|slots| slots.get(index)).map(|(_, ty)| ty.clone())
                };
                settle_pattern(subpattern, field_type, span.clone(), pass, locals)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn settle_pattern_literal(
    literal: &mut HirLiteral,
    expected: Option<&ValkyrieType>,
    span: Range<usize>,
    pass: &SettlePass,
) -> Result<(), HirError> {
    if let HirLiteral::String(capture) = literal {
        match settle_capture(capture, expected, pass, span)? {
            Some(SettledCapture::Char(code)) => *literal = HirLiteral::Character(code),
            Some(SettledCapture::Text(encoding)) => capture.text_encoding = Some(encoding),
            None => {}
        }
    }
    Ok(())
}

/// 结算单个捕获；`Ok(None)` 表示目标与捕获不相容且非默认位——保持未定型，
/// 由校验失败关闭，不得在这里按引号猜一个类型。
fn settle_capture(
    capture: &HirStringLiteral,
    expected: Option<&ValkyrieType>,
    pass: &SettlePass,
    span: Range<usize>,
) -> Result<Option<SettledCapture>, HirError> {
    let Some(target) = expected else {
        return default_capture(capture, span);
    };
    match pass.relations.capture_target_kind(target) {
        CaptureTargetKind::Char => match capture.as_single_char() {
            Some(code) => Ok(Some(SettledCapture::Char(code))),
            None => Err(settle_quote_error(
                QuoteLiteralFailure::CharContextNotRepresentable { reason: representability_reason(capture) },
                span,
            )),
        },
        CaptureTargetKind::Text(encoding) => Ok(Some(SettledCapture::Text(encoding))),
        CaptureTargetKind::Numeric => Ok(capture_binds(capture, CaptureTargetKind::Numeric)),
        CaptureTargetKind::Other => {
            // 开放泛型/自动类型位置按引号默认结算：泛型实参已由调用解析以自然型统一。
            if matches!(target, ValkyrieType::Generic(_) | ValkyrieType::AutoType) {
                default_capture(capture, span)
            }
            else {
                Ok(None)
            }
        }
    }
}

/// 引号默认（最后、只用一次）；无约束 `''`/`'ab'` 默认 char 后报表示错误。
fn default_capture(capture: &HirStringLiteral, span: Range<usize>) -> Result<Option<SettledCapture>, HirError> {
    match capture_quote_default(capture) {
        Some(settled) => Ok(Some(settled)),
        None => {
            Err(settle_quote_error(
                QuoteLiteralFailure::CharDefaultNotRepresentable { reason: representability_reason(capture) },
                span,
            ))
        }
    }
}

fn representability_reason(capture: &HirStringLiteral) -> String {
    match capture_code_points(capture) {
        Some(count) => format!("expected exactly one code point, got {count}"),
        None => "content contains interpolation".to_string(),
    }
}
