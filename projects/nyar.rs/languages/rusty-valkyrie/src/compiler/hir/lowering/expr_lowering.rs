use std::ops::Range;

use ordered_float::OrderedFloat;

use super::*;
use crate::compiler::{
    semantic::operator_members::{
        CARDINAL_SUBSCRIPT_ASSIGN_MEMBER, CARDINAL_SUBSCRIPT_MEMBER, DEREF_READ_MEMBER, FOR_IN_HAS_NEXT_MEMBER, FOR_IN_INTO_ITERATOR_MEMBER,
        FOR_IN_NEXT_MEMBER, FOR_IN_UNWRAP_MEMBER, SUBSCRIPT_ASSIGN_MEMBER, SUBSCRIPT_MEMBER, infix_member_name, prefix_member_name,
    },
    syntax::oak::{
        self as frontend,
        ast::{
            AnonymousClass, AnonymousMicro, AnonymousStructure, ArrayPattern, Block, Break, ClassPattern, Continue, LiteralPattern, MatchArm,
            Pattern, QuoteKind, QuoteLiteral, Raise, Resume, Return, Statement, TermBinaryNode, TermExpression, TermUnaryNode, VariablePattern,
            WildcardPattern, pattern_nodes::ElsePattern,
        },
    },
    types::hir::{CaptureMode, CaptureStorage, HirCapture},
};

use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

/// 表达式 lowering 的最大嵌套深度。
///
/// 守卫只约束递归深度，不改变任何语义；超过上限必须结构化失败，
/// 不能靠扩大线程栈掩盖递归问题。
pub const MAX_EXPRESSION_LOWERING_DEPTH: usize = 24;

thread_local! {
    static EXPRESSION_LOWERING_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// 进入/退出一次表达式 lowering 的 RAII 深度守卫。
struct ExpressionDepthGuard;

impl ExpressionDepthGuard {
    fn enter(span: &SourceSpan) -> Result<Self, HirError> {
        let depth = EXPRESSION_LOWERING_DEPTH.with(|cell| {
            let next = cell.get().saturating_add(1);
            cell.set(next);
            next
        });
        if depth > MAX_EXPRESSION_LOWERING_DEPTH {
            EXPRESSION_LOWERING_DEPTH.with(|cell| cell.set(depth.saturating_sub(1)));
            return Err(HirError::ExpressionNestingExceeded {
                depth,
                limit: MAX_EXPRESSION_LOWERING_DEPTH,
                source: nyar_types::DiagnosticSource { id: "<hir_lowering>".into(), revision: None },
                span: nyar_types::DiagnosticSpan { start: span.span.start as usize, end: span.span.end as usize },
            });
        }
        Ok(Self)
    }
}

impl Drop for ExpressionDepthGuard {
    fn drop(&mut self) {
        EXPRESSION_LOWERING_DEPTH.with(|cell| cell.set(cell.get().saturating_sub(1)));
    }
}

/// 在独立 `#[inline(never)]` 栈帧中执行单个构造的 lowering 体。
///
/// 巨型 `match` 会把所有分支的临时值合并进同一帧；把每个分支体放进
/// 单独的调用帧后，单层递归只承担该分支自身的临时值。
#[inline(never)]
fn lower_deferred<F>(body: F) -> Result<HirExprKind, HirError>
where
    F: FnOnce() -> Result<HirExprKind, HirError>,
{
    body()
}

#[inline(never)]
pub(super) fn lower_block(body: &Block, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirBlock, HirError> {
    let span_range = if body.span.is_empty() { fallback_span } else { frontend::std_range(&body.span) };
    let (statements, tail) = split_block_tail(&body.statements, source_id, span_range.clone())?;
    let expr = tail.map(|expr| lower_term_expression(&expr, source_id, span_range.clone())).transpose()?.map(Box::new);
    Ok(HirBlock { statements, expr, span: with_source(&span_range, source_id) })
}

fn split_block_tail(
    statements: &[Statement],
    source_id: SourceID,
    fallback_span: Range<usize>,
) -> Result<(Vec<HirStatement>, Option<TermExpression>), HirError> {
    if statements.is_empty() {
        return Ok((Vec::new(), None));
    }
    let mut lowered: Vec<HirStatement> = Vec::with_capacity(statements.len());
    for statement in statements {
        lowered.push(lower_statement(statement, source_id, fallback_span.clone())?);
    }
    if let Some(Statement::Expression(expr_stmt)) = statements.last() {
        if !expr_stmt.semi {
            lowered.pop();
            return Ok((lowered, Some(expr_stmt.expr.clone())));
        }
    }
    Ok((lowered, None))
}

fn lower_statement(statement: &Statement, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirStatement, HirError> {
    match statement {
        Statement::Let(binding) => {
            let span_range = if binding.span.is_empty() { fallback_span } else { frontend::std_range(&binding.span) };
            let span = with_source(&span_range, source_id);
            let initializer = match &binding.expr {
                Some(expr) => Some(Box::new(lower_term_expression(expr, source_id, span_range.clone())?)),
                None => None,
            };
            Ok(HirStatement {
                kind: HirStatementKind::Let {
                    is_mutable: has_modifier(&binding.annotations, "mut"),
                    pattern: lower_pattern(&binding.pattern, source_id, span.clone())?,
                    initializer,
                    ty: binding.ty.as_ref().map(lower_type_expression).transpose()?,
                    origin: crate::compiler::types::hir::HirLetOrigin::Ordinary,
                },
                span,
            })
        }
        Statement::Expression(expr_stmt) => {
            let span_range = if expr_stmt.span.is_empty() { fallback_span } else { frontend::std_range(&expr_stmt.span) };
            let span = with_source(&span_range, source_id);
            let expr = lower_statement_expression(&expr_stmt.expr, source_id, span_range, span.clone())?;
            Ok(HirStatement { kind: HirStatementKind::Expr(Box::new(expr)), span })
        }
        Statement::Template(_) => {
            panic!("TGrammar 模板必须在 HIR lowering 前按 arch 展开")
        }
    }
}

fn lower_statement_expression(
    expression: &TermExpression,
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExpr, HirError> {
    match expression {
        TermExpression::Match { scrutinee, arms, span: match_span, .. } => Ok(HirExpr {
            kind: HirExprKind::Case {
                scrutinee: Box::new(lower_term_expression_with_context(scrutinee, source_id, fallback_span.clone(), false)?),
                arms: lower_match_arms(arms, source_id, fallback_span, span.clone(), crate::compiler::syntax::oak::std_range(match_span))?,
            },
            span,
        }),
        _ => lower_term_expression(expression, source_id, fallback_span),
    }
}

fn lower_pattern(pattern: &Pattern, source_id: SourceID, span: SourceSpan) -> Result<HirPattern, HirError> {
    Ok(match pattern {
        Pattern::Wildcard(wildcard) => HirPattern::Wildcard,
        Pattern::Variable(variable) => HirPattern::Variable(HirIdentifier {
            name: Identifier::new(&variable.name.name),
            shadow_index: 0,
            span: with_source(&frontend::std_range(&variable.span), source_id),
        }),
        Pattern::Literal(literal) => HirPattern::Literal(parse_pattern_literal(literal)?),
        Pattern::Type(type_pattern) => HirPattern::Name(lower_name_path(&type_pattern.name)),
        Pattern::Class(class_pattern) => lower_class_pattern(class_pattern, source_id, span)?,
        Pattern::Else(_) => HirPattern::Else,
        // 备选模式 `A | B`：HIR 的 `Or` 是既有模式形态，MIR 与类型检查均已消费；
        // 这里只按源码顺序降低每个分支，不复制 arm body、不另设匹配语义。
        Pattern::Alternative(alternative) => HirPattern::Or(
            alternative
                .patterns
                .iter()
                .map(|nested| lower_pattern(nested, source_id, span.clone()))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Array(array_pattern) => lower_array_pattern(array_pattern, source_id, span)?,
    })
}

fn lower_array_pattern(pattern: &ArrayPattern, source_id: SourceID, span: SourceSpan) -> Result<HirPattern, HirError> {
    // 内置数组类型 `[T]` 的名义 owner 名是 `Array`（`impl_nominal_type_name`），
    // extractor 路径遵循 `<Owner>.extractor` 约定（`canonical_extractor_callee`）。
    let canonical_callee = crate::compiler::types::NamePath::new(vec![Identifier::new("Array"), Identifier::new("extractor")]);
    let prefix = pattern.prefix.iter().map(|item| lower_pattern(item, source_id, span.clone())).collect::<Result<Vec<_>, _>>()?;
    let rest = pattern.rest.as_ref().map(|rest| lower_array_rest_binding(rest, source_id, span.clone()));
    let suffix = pattern.suffix.iter().map(|item| lower_pattern(item, source_id, span.clone())).collect::<Result<Vec<_>, _>>()?;
    Ok(HirPattern::Extractor(HirExtractorPattern::Array { canonical_callee, prefix, rest, suffix, resolved: None }))
}

fn lower_array_rest_binding(pattern: &Pattern, source_id: SourceID, span: SourceSpan) -> HirIdentifier {
    match pattern {
        Pattern::Variable(variable) => HirIdentifier {
            name: Identifier::new(&variable.name.name),
            shadow_index: 0,
            span: with_source(&frontend::std_range(&variable.span), source_id),
        },
        // HIR 的 rest 仅为绑定；`..._` 等无绑定形态落为汇名 `_`，槽位 arity 语义不变。
        _ => HirIdentifier { name: Identifier::new("_"), shadow_index: 0, span },
    }
}

fn lower_class_pattern(pattern: &ClassPattern, source_id: SourceID, span: SourceSpan) -> Result<HirPattern, HirError> {
    let name = lower_name_path(&pattern.name);
    let mut fields = Vec::with_capacity(pattern.fields.len());
    for (field, nested) in &pattern.fields {
        let field_pattern = match nested {
            Some(nested) => lower_pattern(nested, source_id, span.clone())?,
            None => HirPattern::Variable(HirIdentifier { name: Identifier::new(&field.name), shadow_index: 0, span: span.clone() }),
        };
        fields.push((Identifier::new(&field.name), field_pattern));
    }
    let field_ids = vec![None; fields.len()];
    Ok(HirPattern::Object { name: Some(name), type_declaration: None, fields, field_ids, named_fields: false, rest: None })
}

fn parse_pattern_literal(literal: &LiteralPattern) -> Result<HirLiteral, HirError> {
    let value = literal.value.as_str();
    if value == "true" || value == "false" {
        return Ok(HirLiteral::Bool(value == "true"));
    }
    let literal_range = frontend::std_range(&literal.span);
    if let Some(literal) = quote_literal::split_pattern_capture(value, literal_range.start) {
        let prefix_name = literal.prefix.as_ref().map(|prefix| prefix.name.as_str());
        let decoded = quote_literal::decode_quote_capture(&literal, false)?;
        return quote_literal::pattern_literal_value(decoded, prefix_name, literal.quote, literal.quote_count);
    }
    if let Ok(value) = split_numeric_type_suffix(value).0.parse::<i128>() {
        return Ok(HirLiteral::Integer(value));
    }
    if let Ok(value) = split_numeric_type_suffix(value).0.parse::<f64>() {
        return Ok(HirLiteral::Float64(OrderedFloat(value)));
    }
    Ok(HirLiteral::String(crate::compiler::types::hir::HirStringLiteral {
        prefix: None,
        quote_count: 1,
        quote: crate::compiler::types::hir::HirQuoteKind::Double,
        segments: vec![crate::compiler::types::hir::HirStringSegment::Text(value.to_string())],
        text_encoding: None,
    }))
}

#[doc(hidden)]
pub fn lower_term_expression(expression: &TermExpression, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirExpr, HirError> {
    lower_term_expression_with_context(expression, source_id, fallback_span, false)
}

fn lower_term_expression_with_context(
    expression: &TermExpression,
    source_id: SourceID,
    fallback_span: Range<usize>,
    preserve_member_access: bool,
) -> Result<HirExpr, HirError> {
    let span_range = {
        let span = expression.span();
        if span.is_empty() { fallback_span } else { frontend::std_range(&span) }
    };
    let span = with_source(&span_range, source_id);
    let _depth_guard = ExpressionDepthGuard::enter(&span)?;
    let kind = lower_term_expression_body(expression, source_id, span_range, preserve_member_access, span.clone())?;
    Ok(HirExpr { kind, span })
}

/// 表达式 lowering 的构造分派体。
///
/// 独立成 `#[inline(never)]` 函数，避免巨型 `match` 与递归调用点共用同一份
/// 超大栈帧；深度守卫由调用方统一维护。
#[inline(never)]
fn lower_term_expression_body(
    expression: &TermExpression,
    source_id: SourceID,
    span_range: Range<usize>,
    preserve_member_access: bool,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    Ok(match expression {
        TermExpression::ArrayLiteral { items, .. } => lower_deferred(|| {
            Ok(HirExprKind::ArrayLiteral {
                items: items
                    .iter()
                    .map(|item| lower_term_expression_with_context(item, source_id, span_range.clone(), false))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })?,
        TermExpression::TupleLiteral { items, .. } => lower_deferred(|| {
            Ok(HirExprKind::TupleLiteral {
                items: items
                    .iter()
                    .map(|item| lower_term_expression_with_context(item, source_id, span_range.clone(), false))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })?,
        TermExpression::NamePath(path) => lower_name_expression(path, span.clone()),
        TermExpression::QuoteLiteral(literal) => lower_quote_literal_kind(literal, source_id)?,
        TermExpression::UnitLiteral { .. } => HirExprKind::Literal(HirLiteral::Unit),
        TermExpression::IntegerLiteral { value, .. } => {
            HirExprKind::Literal(HirLiteral::Integer(integer_literal_value(split_numeric_type_suffix(value).0, false, span_range.clone())?))
        }
        TermExpression::FloatLiteral { value, .. } => {
            let digits = split_numeric_type_suffix(value).0;
            let parsed = digits.parse::<f64>().unwrap_or_else(|error| panic!("illegal float literal `{value}`: {error}"));
            HirExprKind::Literal(HirLiteral::Float64(OrderedFloat(parsed)))
        }
        TermExpression::BooleanLiteral { value, .. } => HirExprKind::Literal(HirLiteral::Bool(*value)),
        TermExpression::Unary(node) => lower_unary_expression(node, source_id, span_range.clone(), span.clone())?,
        TermExpression::Binary(node) => lower_binary_expression(node, source_id, span_range.clone(), span.clone())?,
        TermExpression::Cast { expr, ty, .. } => lower_deferred(|| {
            Ok(HirExprKind::Cast {
                expr: Box::new(lower_term_expression_with_context(expr, source_id, span_range.clone(), false)?),
                ty: super::lower_type_expression(ty)?,
            })
        })?,
        TermExpression::Turbofish { expr, arguments, .. } => lower_deferred(|| {
            Ok(HirExprKind::GenericApply {
                callee: Box::new(lower_term_expression_with_context(expr, source_id, span_range.clone(), false)?),
                arguments: arguments.iter().map(super::lower_type_expression).collect::<Result<Vec<_>, _>>()?,
            })
        })?,
        // `expr?`：HIR 的 `TryPropagate` 是既有语义节点，ok 变体由后续 HIR 校验阶段解析。
        TermExpression::TryPropagate { expr, .. } => lower_deferred(|| {
            Ok(HirExprKind::TryPropagate {
                inner: Box::new(lower_term_expression_with_context(expr, source_id, span_range.clone(), false)?),
                resolved_ok_variant: None,
            })
        })?,
        TermExpression::ApplyCall { callee, args, .. } => lower_call_expression(callee, args, source_id, span_range.clone(), span.clone())?,
        TermExpression::DotCall { receiver, field, .. } => lower_deferred(|| {
            let object = lower_term_expression_with_context(receiver, source_id, span_range.clone(), false)?;
            let member = field.name.as_str();
            Ok(if let Some(kind) = lower_postfix_effect_member(member, object.clone()) {
                kind
            }
            else {
                HirExprKind::FieldAccess { object: Box::new(object), field: Identifier::new(member), field_id: None, tuple_index: None }
            })
        })?,
        TermExpression::Index { receiver, index, .. } => lower_deferred(|| {
            let receiver = lower_term_expression_with_context(receiver, source_id, span_range.clone(), false)?;
            let index = lower_term_expression_with_context(index, source_id, span_range.clone(), false)?;
            Ok(lower_operator_member_call(
                SUBSCRIPT_MEMBER,
                vec![HirCallArgument::positional(receiver), HirCallArgument::positional(index)],
                span.clone(),
            ))
        })?,
        TermExpression::Offset { receiver, offset, .. } => lower_deferred(|| {
            let receiver = lower_term_expression_with_context(receiver, source_id, span_range.clone(), false)?;
            let offset = lower_term_expression_with_context(offset, source_id, span_range.clone(), false)?;
            Ok(lower_operator_member_call(
                CARDINAL_SUBSCRIPT_MEMBER,
                vec![HirCallArgument::positional(receiver), HirCallArgument::positional(offset)],
                span.clone(),
            ))
        })?,
        TermExpression::Group { expr, .. } => {
            lower_deferred(|| Ok(lower_term_expression_with_context(expr, source_id, span_range.clone(), preserve_member_access)?.kind))?
        }
        TermExpression::Block(block) => {
            lower_deferred(|| Ok(HirExprKind::Block(Box::new(lower_block(block, source_id, span_range.clone())?))))?
        }
        TermExpression::Micro(lambda) => lower_deferred(|| {
            let hir_params =
                lambda.params.iter().map(|param| lower_param(param, source_id, span_range.clone())).collect::<Result<Vec<_>, _>>()?;
            let hir_return_type = lambda.return_type.as_ref().map(lower_type_expression).transpose()?.unwrap_or(ValkyrieType::AutoType);
            let hir_body = lower_block(&lambda.body, source_id, span_range.clone())?;
            Ok(HirExprKind::Lambda {
                generics: Vec::new(),
                params: hir_params,
                return_type: hir_return_type,
                body: Box::new(hir_body),
                captures: Vec::new(),
                hoisted: None,
            })
        })?,
        TermExpression::Object { callee, fields, .. } => lower_object_expression(callee, fields, source_id, span_range.clone(), span.clone())?,
        TermExpression::AnonymousClass(literal) => lower_anonymous_class(literal, source_id, span_range.clone())?,
        TermExpression::AnonymousStructure(literal) => lower_anonymous_structure(literal, source_id, span_range.clone())?,
        TermExpression::If { pattern, condition, then_branch, else_branch, .. } => lower_deferred(|| {
            if let Some(pattern) = pattern {
                Ok(HirExprKind::IfLet {
                    pattern: lower_pattern(pattern, source_id, span.clone())?,
                    scrutinee: Box::new(lower_term_expression_with_context(condition, source_id, span_range.clone(), false)?),
                    then_branch: Box::new(lower_block(then_branch, source_id, span_range.clone())?),
                    else_branch: else_branch.as_ref().map(|body| lower_block(body, source_id, span_range.clone())).transpose()?.map(Box::new),
                })
            }
            else {
                Ok(HirExprKind::If {
                    condition: Box::new(lower_term_expression_with_context(condition, source_id, span_range.clone(), false)?),
                    then_branch: Box::new(lower_block(then_branch, source_id, span_range.clone())?),
                    else_branch: else_branch.as_ref().map(|body| lower_block(body, source_id, span_range.clone())).transpose()?.map(Box::new),
                })
            }
        })?,
        TermExpression::Match { scrutinee, arms, span: match_span, .. } => lower_deferred(|| {
            Ok(HirExprKind::Match {
                scrutinee: Box::new(lower_term_expression_with_context(scrutinee, source_id, span_range.clone(), false)?),
                arms: lower_match_arms(arms, source_id, span_range.clone(), span.clone(), crate::compiler::syntax::oak::std_range(match_span))?,
            })
        })?,
        TermExpression::Loop { label, pattern, condition, body, .. } => lower_deferred(|| {
            let hir_label = label.as_ref().map(|value| Identifier::new(value));
            if let Some(loop_pattern) = pattern {
                // for-in 在 HIR 早期脱糖为迭代协议的普通成员调用，不存在循环专用类型规则；
                // 迭代项类型由 `next() -> Option<Item>` 经 `unwrap` 普通推导得出。
                let iter_source = condition.as_ref().expect("语法不变量：`loop pat in src` 必带迭代源");
                let iterator = lower_term_expression_with_context(iter_source, source_id, span_range.clone(), false)?;
                let pattern = lower_pattern(loop_pattern, source_id, span.clone())?;
                let body = lower_block(body, source_id, span_range.clone())?;
                return Ok(lower_for_in_desugar(hir_label, pattern, iterator, body, span.clone()));
            }
            Ok(HirExprKind::Loop {
                label: hir_label,
                pattern: None,
                iterator: None,
                condition: condition
                    .as_ref()
                    .map(|expr| lower_term_expression_with_context(expr, source_id, span_range.clone(), false))
                    .transpose()?
                    .map(|e| Box::new(e)),
                body: Box::new(lower_block(body, source_id, span_range.clone())?),
            })
        })?,
        TermExpression::Return(node) => lower_deferred(|| {
            Ok(HirExprKind::Return(
                node.base
                    .as_ref()
                    .map(|expr| lower_term_expression_with_context(expr, source_id, span_range.clone(), false))
                    .transpose()?
                    .map(|e| Box::new(e)),
            ))
        })?,
        TermExpression::Break(node) => lower_deferred(|| {
            Ok(HirExprKind::Break {
                label: node.label.as_ref().map(|value| Identifier::new(value)),
                expr: node
                    .base
                    .as_ref()
                    .map(|expr| lower_term_expression_with_context(expr, source_id, span_range.clone(), false))
                    .transpose()?
                    .map(|e| Box::new(e)),
            })
        })?,
        TermExpression::Continue(node) => HirExprKind::Continue { label: node.label.as_ref().map(|value| Identifier::new(value)) },
        TermExpression::Raise(node) => lower_deferred(|| {
            Ok(HirExprKind::Raise(Box::new(lower_optional_term_expression(node.base.as_ref(), source_id, span_range.clone(), span.clone())?)))
        })?,
        TermExpression::Resume(node) => lower_deferred(|| {
            Ok(HirExprKind::Resume(Box::new(lower_optional_term_expression(node.base.as_ref(), source_id, span_range.clone(), span.clone())?)))
        })?,
        TermExpression::Yield { expr, .. } => lower_deferred(|| {
            Ok(HirExprKind::Yield(
                expr.as_ref()
                    .map(|value| lower_term_expression_with_context(value, source_id, span_range.clone(), false))
                    .transpose()?
                    .map(|e| Box::new(e)),
            ))
        })?,
        TermExpression::Catch { expr, arms, span: catch_span, .. } => lower_deferred(|| {
            Ok(HirExprKind::Catch {
                expr: Box::new(lower_term_expression_with_context(expr, source_id, span_range.clone(), false)?),
                arms: lower_match_arms(arms, source_id, span_range.clone(), span.clone(), crate::compiler::syntax::oak::std_range(catch_span))?,
            })
        })?,
        TermExpression::With { base, updates, .. } => lower_deferred(|| {
            let object = lower_term_expression_with_context(base, source_id, span_range.clone(), false)?;
            let mut hir_updates: Vec<HirExpr> = Vec::with_capacity(updates.len());
            for (field, value) in updates {
                let value = lower_term_expression_with_context(value, source_id, span_range.clone(), false)?;
                hir_updates.push(HirExpr {
                    kind: HirExprKind::FieldInit { name: Identifier::new(&field.name), value: Box::new(value), field_id: None },
                    span: span.clone(),
                });
            }
            Ok(HirExprKind::Call {
                callee: Box::new(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new("with")])), span: span.clone() }),
                args: vec![HirCallArgument::positional(object)]
                    .into_iter()
                    .chain(hir_updates.into_iter().map(|expr| HirCallArgument::positional(expr)))
                    .collect(),
                resolved: None,
            })
        })?,
        TermExpression::SuperCall { method, args, .. } => lower_deferred(|| {
            let callee = HirExpr {
                kind: HirExprKind::Path(NamePath::new(vec![Identifier::new("super"), Identifier::new(&method.name)])),
                span: span.clone(),
            };
            let arg_exprs: Vec<_> = args
                .iter()
                .map(|arg| lower_term_expression_with_context(arg, source_id, span_range.clone(), false))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(HirCallArgument::positional)
                .collect();
            Ok(lower_canonical_call_arguments(callee, arg_exprs))
        })?,
    })
}

#[inline(never)]
fn lower_unary_expression(
    node: &TermUnaryNode,
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    if matches!(node.operator, ValkyrieTokenType::Minus) {
        if let TermExpression::IntegerLiteral { value, .. } = &node.base {
            // 按"符号 + 量级"结合：量级先以 `u64` 承载，结果保持未定型 `i128`，
            // 目标宽度在上下文处一次结算；本层不提前收窄到 `i64`。
            let folded = integer_literal_value(value, true, frontend::std_range(&node.base.span()))?;
            return Ok(HirExprKind::Literal(HirLiteral::Integer(folded)));
        }
    }
    if matches!(node.operator, ValkyrieTokenType::Star) {
        let base = lower_term_expression_with_context(&node.base, source_id, fallback_span, false)?;
        Ok(lower_method_call_kind(DEREF_READ_MEMBER, vec![HirCallArgument::positional(base)], span))
    }
    else {
        let base = lower_term_expression_with_context(&node.base, source_id, fallback_span, false)?;
        Ok(lower_operator_member_call(&prefix_member_name(&node.operator), vec![HirCallArgument::positional(base)], span))
    }
}

#[inline(never)]
fn lower_binary_expression(
    node: &TermBinaryNode,
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    let lhs = &node.lhs;
    let rhs = &node.rhs;
    match node.operator {
        ValkyrieTokenType::Eq => {
            if let TermExpression::Index { receiver, index, .. } | TermExpression::Offset { receiver, offset: index, .. } = lhs {
                let receiver = lower_term_expression_with_context(receiver, source_id, fallback_span.clone(), false)?;
                let index = lower_term_expression_with_context(index, source_id, fallback_span.clone(), false)?;
                let rhs = lower_term_expression_with_context(rhs, source_id, fallback_span, false)?;
                return Ok(lower_operator_member_call(
                    if matches!(lhs, TermExpression::Index { .. }) { SUBSCRIPT_ASSIGN_MEMBER } else { CARDINAL_SUBSCRIPT_ASSIGN_MEMBER },
                    vec![HirCallArgument::positional(receiver), HirCallArgument::positional(index), HirCallArgument::positional(rhs)],
                    span,
                ));
            }
            let target = lower_term_expression_with_context(lhs, source_id, fallback_span.clone(), true)?;
            let value = Box::new(lower_term_expression_with_context(rhs, source_id, fallback_span, false)?);
            match target.kind {
                HirExprKind::Variable(identifier) => Ok(HirExprKind::Assign { target: identifier.name, value }),
                HirExprKind::FieldAccess { object, field, field_id, .. } => Ok(HirExprKind::StoreField { object, field, value, field_id }),
                _ => panic!("Oak 已验证的赋值位置不能降低为其他表达式"),
            }
        }
        ValkyrieTokenType::AndAnd => Ok(lower_short_circuit_and(lhs, rhs, source_id, fallback_span, span)?),
        ValkyrieTokenType::OrOr => Ok(lower_short_circuit_or(lhs, rhs, source_id, fallback_span, span)?),
        _ => {
            let lhs = lower_term_expression_with_context(lhs, source_id, fallback_span.clone(), false)?;
            let rhs = lower_term_expression_with_context(rhs, source_id, fallback_span, false)?;
            Ok(lower_operator_member_call(
                &infix_member_name(&node.operator),
                vec![HirCallArgument::positional(lhs), HirCallArgument::positional(rhs)],
                span,
            ))
        }
    }
}

#[inline(never)]
fn lower_object_expression(
    callee: &TermExpression,
    fields: &[(frontend::ast::Identifier, Option<TermExpression>)],
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    let (name_callee, generic_arguments) = match callee {
        TermExpression::Turbofish { expr, arguments, .. } => {
            (expr.as_ref(), arguments.iter().map(super::lower_type_expression).collect::<Result<Vec<_>, _>>()?)
        }
        _ => (callee, Vec::new()),
    };
    let path = extract_name_path(name_callee).unwrap_or_else(|| NamePath::new(vec![Identifier::new("_")]));
    let args: Vec<_> = fields
        .iter()
        .map(|(field, value)| -> Result<_, HirError> {
            let value = match value.as_ref() {
                Some(expr) => lower_term_expression_with_context(expr, source_id, fallback_span.clone(), false)?,
                None => HirExpr {
                    kind: HirExprKind::Variable(HirIdentifier { name: Identifier::new(&field.name), shadow_index: 0, span: span.clone() }),
                    span: span.clone(),
                },
            };
            Ok(HirExpr {
                kind: HirExprKind::FieldInit { name: Identifier::new(&field.name), value: Box::new(value), field_id: None },
                span: span.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HirExprKind::Construct { path, generic_arguments, args, resolved: None })
}

fn lower_anonymous_class(literal: &AnonymousClass, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirExprKind, HirError> {
    lower_anonymous_literal_fields(false, &literal.span, &literal.parents, &literal.body, &literal.captures, source_id, fallback_span)
}

fn lower_anonymous_structure(literal: &AnonymousStructure, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirExprKind, HirError> {
    lower_anonymous_literal_fields(true, &literal.span, &literal.parents, &literal.body, &literal.captures, source_id, fallback_span)
}

fn lower_anonymous_literal_fields(
    is_value_type: bool,
    literal_span: &frontend::ast::Span,
    parents: &[frontend::ast::Parent],
    body: &frontend::ast::StructureBody,
    captures: &[frontend::ast::Identifier],
    source_id: SourceID,
    fallback_span: Range<usize>,
) -> Result<HirExprKind, HirError> {
    let default_fields: Vec<_> = body
        .fields
        .iter()
        .filter_map(|field| {
            field.default.as_ref().map(|value| -> Result<_, HirError> {
                Ok((
                    Identifier::new(&field.name.name),
                    Box::new(lower_term_expression_with_context(value, source_id, fallback_span.clone(), false)?),
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let methods = body.methods.iter().map(|method| lower_inline_object_method(method, source_id)).collect::<Result<Vec<_>, _>>()?;
    Ok(HirExprKind::AnonymousClass {
        is_value_type,
        parents: parents.iter().map(lower_parent).collect(),
        fields: default_fields,
        methods,
        captures: captures
            .iter()
            .map(|capture| HirCapture {
                identifier: HirIdentifier {
                    name: Identifier::new(&capture.name),
                    shadow_index: 0,
                    span: with_source(&frontend::std_range(literal_span), source_id),
                },
                ty: ValkyrieType::Unit,
                mode: CaptureMode::ByReference,
                is_mutable: false,
                is_write: false,
                storage_hint: CaptureStorage::default(),
            })
            .collect(),
        class_name: None,
    })
}

fn lower_inline_object_method(method: &frontend::ast::MethodDeclaration, source_id: SourceID) -> Result<HirFunction, HirError> {
    let span_range = frontend::std_range(&method.span);
    Ok(HirFunction {
        declaration: None,
        instance: None,
        name: Identifier::new(&method.name.name),
        declaring_namespace: NamePath::default(),
        doc: lower_documentation(&method.annotations),
        annotations: method
            .annotations
            .iter()
            .map(|attribute| lower_attribute(attribute, source_id, span_range.clone()))
            .collect::<Result<Vec<_>, _>>()?,
        generics: lower_generic_parameters(&method.generics),
        where_constraints: Vec::new(),
        params: lower_method_params(method, source_id)?,
        return_type: method.return_type.as_ref().map(lower_type_expression).transpose()?.unwrap_or(ValkyrieType::Unit),
        body: method.body.as_ref().map(|body| lower_block(body, source_id, span_range.clone())).transpose()?.unwrap_or_else(|| HirBlock {
            statements: Vec::new(),
            expr: None,
            span: with_source(&span_range, source_id),
        }),
        span: with_source(&span_range, source_id),
        visibility: lower_visibility(&method.annotations),
        is_abstract: method.body.is_none() || has_modifier(&method.annotations, "abstract"),
        is_final: has_modifier(&method.annotations, "final"),
        is_virtual: has_modifier(&method.annotations, "virtual"),
        is_override: has_modifier(&method.annotations, "override"),
    })
}

fn lower_postfix_effect_member(member: &str, object: HirExpr) -> Option<HirExprKind> {
    match member {
        "await" => Some(HirExprKind::Await(Box::new(object))),
        "awake" => Some(HirExprKind::Awake(Box::new(object))),
        "block" => Some(HirExprKind::BlockOn(Box::new(object))),
        _ => None,
    }
}

fn lower_match_arms(
    arms: &[MatchArm],
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
    match_span: Range<usize>,
) -> Result<Vec<HirMatchArm>, HirError> {
    if let Some(source) = super::source_context::hir_lower_source() {
        if super::match_pattern_recovery::match_needs_source_recovery(&source, match_span.clone(), arms) {
            if let Ok(recovered) = super::match_pattern_recovery::recover_match_arms(&source, match_span.clone()) {
                let result: Result<Vec<_>, _> = recovered
                    .into_iter()
                    .map(|(pattern, body)| -> Result<_, HirError> {
                        Ok(HirMatchArm {
                            pattern,
                            guard: None,
                            body: Box::new(lower_term_expression_with_context(&body, source_id, fallback_span.clone(), false)?),
                        })
                    })
                    .collect();
                return result;
            }
        }
    }
    let source_snapshot = super::source_context::hir_lower_source();
    // 区间臂的源码文本必须从 match 整体切分获得：`case 2..=8:` 的 arm span 从 `8`
    // 起算，`2..=` 已丢，无法按 `arm.span` 恢复。只有列表与臂数一致时才按索引采用。
    let range_texts = source_snapshot
        .as_deref()
        .and_then(|source| super::match_pattern_recovery::recovered_range_pattern_texts(source, match_span.clone()))
        .filter(|texts| texts.len() == arms.len());
    arms.iter()
        .enumerate()
        .map(|(index, arm)| -> Result<_, HirError> {
            let range_text = range_texts.as_ref().and_then(|texts| texts.get(index)).and_then(Option::as_deref);
            let pattern = if let Some(text) = range_text {
                match super::match_pattern_recovery::range_pattern_from_text(text) {
                    Some(pattern) => pattern,
                    None => lower_pattern(&arm.pattern, source_id, span.clone())?,
                }
            }
            else {
                source_snapshot
                    .as_deref()
                    .and_then(|source| super::match_pattern_recovery::recover_arm_object_pattern(source, arm, span.clone()))
                    .map_or_else(|| lower_pattern(&arm.pattern, source_id, span.clone()), Ok)?
            };
            let guard = arm
                .guard
                .as_ref()
                .map(|guard_expr| lower_term_expression_with_context(guard_expr, source_id, fallback_span.clone(), false))
                .transpose()?
                .map(Box::new);
            let body = Box::new(lower_term_expression_with_context(&arm.body, source_id, fallback_span.clone(), false)?);
            Ok(HirMatchArm { pattern, guard, body })
        })
        .collect()
}

#[inline(never)]
fn lower_call_expression(
    callee: &TermExpression,
    args: &[TermExpression],
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    let mut lowered_args: Vec<_> = args
        .iter()
        .map(|arg| -> Result<_, HirError> {
            Ok(HirCallArgument::positional(lower_term_expression_with_context(arg, source_id, fallback_span.clone(), false)?))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(path) = extract_name_path(callee) {
        if path.parts().len() == 1 && lowered_args.len() == 1 {
            if let Some(ty) = crate::compiler::hir::numeric_builtin_cast_type(path.parts()[0].as_str()) {
                let expr = lowered_args.pop().expect("one arg").value;
                return Ok(HirExprKind::Cast { expr: Box::new(expr), ty });
            }
        }
        let callee_expr = HirExpr { kind: HirExprKind::Path(path), span: span.clone() };
        return Ok(lower_canonical_call_arguments(callee_expr, lowered_args));
    }
    let callee_expr = lower_term_expression_with_context(callee, source_id, fallback_span, true)?;
    Ok(lower_canonical_call_arguments(callee_expr, lowered_args))
}

fn lower_method_call_kind(member: &str, args: Vec<HirCallArgument>, span: SourceSpan) -> HirExprKind {
    lower_canonical_call_arguments(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new(member)])), span: span.clone() }, args)
}

/// AST 运算符语法糖在 HIR 早期展开为接收者的普通成员调用。
///
/// 成员名与运算符声明的 HIR 成员名同为 `{fixity} {lexeme}`（见 `operator_member_name`）：
/// `a + b` 展开为接收者 `a` 的 `infix +` 调用，`a[i]` 展开为 `a` 的 `suffix [ ]` 调用。
/// HIR/MIR/LIR 不再持有 unary/binary/operator 语义类别，也不存在运算符身份表。
fn lower_operator_member_call(member: &str, args: Vec<HirCallArgument>, span: SourceSpan) -> HirExprKind {
    lower_canonical_call_arguments(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new(member)])), span }, args)
}

fn lower_canonical_call_arguments(callee: HirExpr, args: Vec<HirCallArgument>) -> HirExprKind {
    HirExprKind::Call { callee: Box::new(callee), args, resolved: None }
}

/// for-in 脱糖：`loop pat in coll { body }` 展开为迭代协议的普通成员调用序列——
///
/// ```text
/// {
///     let mut __for_in_iterator = coll.into_iterator()
///     loop {
///         if !__for_in_iterator.has_next() { break }
///         let pat = __for_in_iterator.next().unwrap()
///         body
///     }
/// }
/// ```
///
/// 迭代项类型由 `Iterator::next() -> Option<Item>` 经 `unwrap` 的普通推导得出：
/// 集合是 `IntoIterator` 的哪个实现、返回什么 `Item`，全部交给普通成员解析按
/// 声明身份冻结，编译器不持有任何集合形状表。脱糖节点共享原循环的 span。
fn lower_for_in_desugar(label: Option<Identifier>, pattern: HirPattern, iterator: HirExpr, body: HirBlock, span: SourceSpan) -> HirExprKind {
    const TEMP: &str = "__for_in_iterator";
    let temp_ident = |span: &SourceSpan| HirIdentifier { name: Identifier::new(TEMP), shadow_index: 0, span: span.clone() };
    let temp_var = |span: &SourceSpan| HirExpr { kind: HirExprKind::Variable(temp_ident(span)), span: span.clone() };
    // 具名方法与 `x.m(args)` 同形：`FieldAccess` 作 callee；运算符才走 `{fixity} {lexeme}` 成员名。
    let dot_call = |object: HirExpr, member: &str, span: &SourceSpan| HirExpr {
        kind: HirExprKind::Call {
            callee: Box::new(HirExpr {
                kind: HirExprKind::FieldAccess { object: Box::new(object), field: Identifier::new(member), field_id: None, tuple_index: None },
                span: span.clone(),
            }),
            args: Vec::new(),
            resolved: None,
        },
        span: span.clone(),
    };
    let operator_call = |member: &str, args: Vec<HirExpr>, span: &SourceSpan| HirExpr {
        kind: HirExprKind::Call {
            callee: Box::new(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new(member)])), span: span.clone() }),
            args: args.into_iter().map(HirCallArgument::positional).collect(),
            resolved: None,
        },
        span: span.clone(),
    };
    let bind_iterator = HirStatement {
        kind: HirStatementKind::Let {
            is_mutable: true,
            pattern: HirPattern::Variable(temp_ident(&span)),
            initializer: Some(Box::new(dot_call(iterator, FOR_IN_INTO_ITERATOR_MEMBER, &span))),
            ty: None,
            // 这是脱糖节点本身的结构化身份：解析期只按该标记结算迭代协议义务。
            origin: crate::compiler::types::hir::HirLetOrigin::ForInIterator,
        },
        span: span.clone(),
    };
    let break_stmt = HirStatement {
        kind: HirStatementKind::Expr(Box::new(HirExpr { kind: HirExprKind::Break { label: None, expr: None }, span: span.clone() })),
        span: span.clone(),
    };
    let guard = HirStatement {
        kind: HirStatementKind::Expr(Box::new(HirExpr {
            kind: HirExprKind::If {
                condition: Box::new(operator_call("prefix !", vec![dot_call(temp_var(&span), FOR_IN_HAS_NEXT_MEMBER, &span)], &span)),
                then_branch: Box::new(HirBlock { statements: vec![break_stmt], expr: None, span: span.clone() }),
                else_branch: None,
            },
            span: span.clone(),
        })),
        span: span.clone(),
    };
    let bind_item = HirStatement {
        kind: HirStatementKind::Let {
            is_mutable: false,
            pattern,
            initializer: Some(Box::new(dot_call(dot_call(temp_var(&span), FOR_IN_NEXT_MEMBER, &span), FOR_IN_UNWRAP_MEMBER, &span))),
            ty: None,
            origin: crate::compiler::types::hir::HirLetOrigin::Ordinary,
        },
        span: span.clone(),
    };
    let mut loop_statements = Vec::with_capacity(body.statements.len() + 2);
    loop_statements.push(guard);
    loop_statements.push(bind_item);
    loop_statements.extend(body.statements);
    let loop_span = body.span.clone();
    let inner_loop = HirExpr {
        kind: HirExprKind::Loop {
            label,
            pattern: None,
            iterator: None,
            condition: None,
            body: Box::new(HirBlock { statements: loop_statements, expr: body.expr, span: loop_span }),
        },
        span: span.clone(),
    };
    let outer_span = span.clone();
    HirExprKind::Block(Box::new(HirBlock {
        statements: vec![bind_iterator, HirStatement { kind: HirStatementKind::Expr(Box::new(inner_loop)), span }],
        expr: None,
        span: outer_span,
    }))
}

fn lower_short_circuit_and(
    lhs: &TermExpression,
    rhs: &TermExpression,
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    let condition = lower_term_expression_with_context(lhs, source_id, fallback_span.clone(), false)?;
    let rhs_expr = lower_term_expression_with_context(rhs, source_id, fallback_span, false)?;
    Ok(HirExprKind::If {
        condition: Box::new(condition),
        then_branch: Box::new(HirBlock { statements: Vec::new(), expr: Some(Box::new(rhs_expr)), span: span.clone() }),
        else_branch: Some(Box::new(HirBlock {
            statements: Vec::new(),
            expr: Some(Box::new(HirExpr { kind: HirExprKind::Literal(HirLiteral::Bool(false)), span: span.clone() })),
            span,
        })),
    })
}

fn lower_short_circuit_or(
    lhs: &TermExpression,
    rhs: &TermExpression,
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExprKind, HirError> {
    let condition = lower_term_expression_with_context(lhs, source_id, fallback_span.clone(), false)?;
    let rhs_expr = lower_term_expression_with_context(rhs, source_id, fallback_span, false)?;
    Ok(HirExprKind::If {
        condition: Box::new(condition),
        then_branch: Box::new(HirBlock {
            statements: Vec::new(),
            expr: Some(Box::new(HirExpr { kind: HirExprKind::Literal(HirLiteral::Bool(true)), span: span.clone() })),
            span: span.clone(),
        }),
        else_branch: Some(Box::new(HirBlock { statements: Vec::new(), expr: Some(Box::new(rhs_expr)), span })),
    })
}

fn lower_name_expression(path: &frontend::ast::NamePath, span: SourceSpan) -> HirExprKind {
    let path = lower_name_path(path);
    if path.parts().len() == 1 {
        // `null` 是语言的可空空值字面量，不是标识符绑定；身份必须在 HIR 冻结，
        // 下游不得再按拼写判定。
        if path.parts()[0].as_str() == "null" {
            return HirExprKind::Literal(HirLiteral::Null);
        }
        HirExprKind::Variable(HirIdentifier { name: path.parts()[0].clone(), shadow_index: 0, span })
    }
    else {
        HirExprKind::Path(path)
    }
}

fn lower_quote_literal_kind(literal: &QuoteLiteral, source_id: SourceID) -> Result<HirExprKind, HirError> {
    // 捕获事实来自 Oak：内容未解码、模板未分段；前缀解释、转义解码与模板解析
    // 由 `quote_literal` 模块按合同完成，失败结构化带原始源区间。
    let decoded = quote_literal::decode_quote_capture(literal, true)?;
    let prefix = literal.prefix.as_ref().map(|prefix| Identifier::new(&prefix.name));
    Ok(HirExprKind::Literal(match decoded {
        quote_literal::DecodedQuoteCapture::Byte(value) => HirLiteral::Byte(value),
        // 退役变体无构造点；透传仅保持穷尽匹配（定型已移交 `quote_settle`）。
        quote_literal::DecodedQuoteCapture::Character(value) => HirLiteral::Character(value),
        quote_literal::DecodedQuoteCapture::PlainText(text) => HirLiteral::String(crate::compiler::types::hir::HirStringLiteral {
            prefix,
            quote_count: literal.quote_count,
            quote: quote_literal::hir_quote_kind(literal.quote),
            segments: vec![crate::compiler::types::hir::HirStringSegment::Text(text)],
            text_encoding: None,
        }),
        quote_literal::DecodedQuoteCapture::Template(parts) => HirLiteral::String(crate::compiler::types::hir::HirStringLiteral {
            prefix,
            quote_count: literal.quote_count,
            quote: quote_literal::hir_quote_kind(literal.quote),
            text_encoding: None,
            segments: parts
                .into_iter()
                .map(|part| -> Result<_, HirError> {
                    match part {
                        quote_literal::QuoteTemplatePart::Text(text) => Ok(crate::compiler::types::hir::HirStringSegment::Text(text)),
                        quote_literal::QuoteTemplatePart::Interpolation { expr, is_fluent, span } => {
                            Ok(crate::compiler::types::hir::HirStringSegment::Interpolation {
                                expr: lower_term_expression_with_context(&expr, source_id, span, false)?,
                                is_fluent,
                            })
                        }
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }))
}

/// 拆掉数字字面量尾部的类型后缀（`0.1f64` → `0.1`、`42i64` → `42`）。
///
/// 后缀由 lexer 只在数字 token 内接受；HIR 只做剥离，不改数值。当前字面量载体
/// 只有 `Integer64`/`Float64`，因此只识别 `i64`/`f64`；十六进制/二进制/八进制
/// 前缀内的字母不是后缀，不能剥离。
pub(crate) fn split_numeric_type_suffix(text: &str) -> (&str, Option<&'static str>) {
    if text.starts_with("0x")
        || text.starts_with("0X")
        || text.starts_with("0b")
        || text.starts_with("0B")
        || text.starts_with("0o")
        || text.starts_with("0O")
    {
        return (text, None);
    }
    for suffix in ["f64", "i64"] {
        if let Some(digits) = text.strip_suffix(suffix)
            && !digits.is_empty()
        {
            return (digits, Some(suffix));
        }
    }
    (text, None)
}

/// 整数字面量按"符号 + 量级"解释：量级用 `u64` 承载，符号在结合时应用。
/// 结果以 `i128` 精确返回，覆盖完整 `u64` 正半域；目标宽度与范围由上下文
/// 在 `integer_literal::settle_integer` 一次结算，本函数不做 `i64` 收窄。
fn integer_literal_value(text: &str, negative: bool, span: Range<usize>) -> Result<i128, HirError> {
    let magnitude = parse_integer_literal_magnitude(text)
        .map_err(|()| integer_literal_out_of_range(text, negative, None, "unsigned_64_magnitude", span.clone()))?;
    let signed = if negative { -(magnitude as i128) } else { magnitude as i128 };
    Ok(signed)
}

fn integer_literal_out_of_range(text: &str, negative: bool, magnitude: Option<u64>, range: &'static str, span: Range<usize>) -> HirError {
    let literal = if negative { format!("-{text}") } else { text.to_string() };
    let magnitude = magnitude.map_or_else(|| text.to_string(), |value| value.to_string());
    HirError::IntegerLiteralOutOfRange {
        literal,
        magnitude,
        range: range.to_string(),
        source: nyar_types::DiagnosticSource { id: "<hir_lowering>".into(), revision: None },
        span: nyar_types::DiagnosticSpan { start: span.start, end: span.end },
    }
}

fn parse_integer_literal_magnitude(text: &str) -> Result<u64, ()> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).map_err(|_| ());
    }
    if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        return u64::from_str_radix(bin, 2).map_err(|_| ());
    }
    if let Some(oct) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
        return u64::from_str_radix(oct, 8).map_err(|_| ());
    }
    text.parse::<u64>().map_err(|_| ())
}

pub(super) fn extract_name_path(expression: &TermExpression) -> Option<NamePath> {
    match expression {
        TermExpression::NamePath(path) => Some(lower_name_path(path)),
        TermExpression::DotCall { receiver, field, .. } => {
            let mut parts = extract_name_path(receiver)?.parts().to_vec();
            parts.push(Identifier::new(&field.name));
            Some(NamePath::new(parts))
        }
        TermExpression::QuoteLiteral(literal) if literal.quote == QuoteKind::Double => {
            let raw = plain_quote_capture_text(literal)?;
            Some(NamePath::new(raw.split("::").filter(|part| !part.is_empty()).map(Identifier::new).collect()))
        }
        _ => None,
    }
}

fn lower_optional_term_expression(
    expression: Option<&TermExpression>,
    source_id: SourceID,
    fallback_span: Range<usize>,
    span: SourceSpan,
) -> Result<HirExpr, HirError> {
    match expression {
        Some(expr) => lower_term_expression_with_context(expr, source_id, fallback_span, false),
        None => Ok(HirExpr { kind: HirExprKind::Literal(HirLiteral::Unit), span }),
    }
}

fn plain_quote_capture_text(literal: &QuoteLiteral) -> Option<String> {
    match quote_literal::decode_quote_capture(literal, false).ok()? {
        quote_literal::DecodedQuoteCapture::PlainText(text) => Some(text),
        _ => None,
    }
}
