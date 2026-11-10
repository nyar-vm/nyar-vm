//! 从源码恢复 Oak 尚未完整降低的 `match`/`catch` arm 模式。

use std::ops::Range;

use nyar_types::DiagnosticSpan;

use crate::compiler::{
    hir::lowering::HirError,
    syntax::oak::{
        ParseError,
        ast::{MatchArm, StatementNode, TermExpression},
        parse_source_with_identity,
    },
    types::{
        Identifier, NamePath, SourceSpan,
        hir::{HirIdentifier, HirLiteral, HirPattern},
    },
};

use super::errors::MatchRecoveryStage;

use ordered_float::OrderedFloat;

/// Oak 产出的 match arm 是否需从源码恢复。
pub fn match_needs_source_recovery(source: &str, match_span: Range<usize>, arms: &[MatchArm]) -> bool {
    let expected = count_match_arms_in_source(source, match_span.clone());
    if expected == 0 {
        return false;
    }
    if expected > arms.len() {
        return true;
    }
    if arms.iter().any(|arm| broken_wildcard_arm(source, arm) || oak_lost_case_pattern(source, arm)) {
        return true;
    }
    // Oak 的 `Pattern` 没有区间变体，且 `case 2..=8:` 的 arm span 会从 `8` 起算
    // （`2..=` 被解析器当作前导表达式吞掉），不能靠 `arm.span` 判定；必须直接从 match
    // 源码切分臂文本。区间必须进源码恢复，否则会退化成等值比较。
    source_match_has_range_pattern(source, match_span)
}

/// 直接按 match 源码切分臂文本，判定是否存在区间模式。
fn source_match_has_range_pattern(source: &str, match_span: Range<usize>) -> bool {
    let Some(interior) = extract_match_block_interior(source, match_span)
    else {
        return false;
    };
    split_match_arms(&interior).into_iter().any(|(pattern, _)| parse_range_pattern(&pattern).is_some())
}

/// 按 match 源码顺序返回每臂的区间模式文本（`None` 表示该臂不是区间）。
///
/// 逐臂路径不能用 `arm.span`：`case 2..=8:` 的 span 从 `8` 起算，前导 `2..=` 丢失。
/// 返回列表长度与源码臂数一致；调用方只有在它与 Oak `arms` 等长时才按索引采用。
pub fn recovered_range_pattern_texts(source: &str, match_span: Range<usize>) -> Option<Vec<Option<String>>> {
    let interior = extract_match_block_interior(source, match_span)?;
    Some(
        split_match_arms(&interior)
            .into_iter()
            .map(|(pattern, _)| if parse_range_pattern(&pattern).is_some() { Some(pattern) } else { None })
            .collect(),
    )
}

/// Oak `parse_match_pattern` 对个别原子仍可能落成 Wildcard，`case 7` 等需按源码恢复。
fn oak_lost_case_pattern(source: &str, arm: &MatchArm) -> bool {
    if !matches!(arm.pattern, oak_valkyrie::ast::Pattern::Wildcard(_)) {
        return false;
    }
    source.get(arm.span.start..arm.span.end).and_then(extract_case_pattern_text).is_some_and(|pattern| !pattern.is_empty() && pattern != "_")
}

fn extract_case_pattern_text(arm_text: &str) -> Option<String> {
    let trimmed = arm_text.trim_start();
    if !trimmed.starts_with("case ") {
        return None;
    }
    let after_case = trimmed[5..].trim_start();
    let colon = find_pattern_colon(after_case, 0);
    if colon == 0 {
        return None;
    }
    Some(after_case[..colon].trim().to_string())
}

/// 从 arm 源码恢复 Oak 丢失的对象模式字段名。
///
/// Oak 的 `parse_match_pattern` 把花括号字段名当作裸标识符消费，builder 只保留首个标识符，
/// 字段整体丢失（`Pattern::Class` 退化成 `Pattern::Type`）。这里只替换模式，不重解析臂体；
/// 字段名按源码写法保留，供 HIR 按声明字段名绑定。
pub fn recover_arm_object_pattern(source: &str, arm: &MatchArm, span: SourceSpan) -> Option<HirPattern> {
    if !matches!(arm.pattern, oak_valkyrie::ast::Pattern::Type(_)) {
        return None;
    }
    let pattern_text = source.get(arm.span.start..arm.span.end).and_then(extract_case_pattern_text)?;
    parse_recovered_object_pattern(&pattern_text, &span)
}

fn parse_recovered_object_pattern(text: &str, span: &SourceSpan) -> Option<HirPattern> {
    let text = text.trim();
    let open = text.find('{')?;
    let close = text.rfind('}')?;
    if close < open {
        return None;
    }
    let head = text[..open].trim();
    if !is_name_path_text(head) {
        return None;
    }
    let mut fields = Vec::new();
    for raw_field in split_top_level(&text[open + 1..close], ',') {
        let raw_field = raw_field.trim();
        if raw_field.is_empty() {
            return None;
        }
        let (field_name, nested) = match find_top_level(raw_field, ':') {
            Some(index) => (raw_field[..index].trim(), Some(raw_field[index + 1..].trim())),
            None => (raw_field, None),
        };
        if !is_plain_identifier(field_name) {
            return None;
        }
        let identifier = Identifier::new(field_name);
        let pattern = match nested {
            Some(nested) if !nested.is_empty() => parse_recovered_field_pattern(nested, span)?,
            _ => HirPattern::Variable(HirIdentifier { name: identifier.clone(), shadow_index: 0, span: span.clone() }),
        };
        fields.push((identifier, pattern));
    }
    if fields.is_empty() {
        return None;
    }
    let field_ids = vec![None; fields.len()];
    Some(HirPattern::Object {
        name: Some(NamePath::new(head.split('.').map(Identifier::new).collect())),
        type_declaration: None,
        fields,
        field_ids,
        named_fields: true,
        rest: None,
    })
}

fn parse_recovered_field_pattern(text: &str, span: &SourceSpan) -> Option<HirPattern> {
    let text = text.trim();
    if text == "_" {
        return Some(HirPattern::Wildcard);
    }
    if text.contains('{') {
        return parse_recovered_object_pattern(text, span);
    }
    if let Some(literal) = parse_hir_literal(text) {
        return Some(HirPattern::Literal(literal));
    }
    if is_plain_identifier(text) {
        let name = Identifier::new(text);
        if text.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
            return Some(HirPattern::Name(NamePath::new(vec![name])));
        }
        return Some(HirPattern::Variable(HirIdentifier { name, shadow_index: 0, span: span.clone() }));
    }
    None
}

fn is_name_path_text(text: &str) -> bool {
    !text.is_empty() && text.split('.').all(is_plain_identifier)
}

fn is_plain_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// 按顶层分隔符切分，跳过括号/花括号/方括号与引号内的分隔符。
fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut start = 0usize;
    for (index, ch) in text.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            }
            else if ch == '\\' {
                escaped = true;
            }
            else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 && ch == separator => {
                parts.push(&text[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// 顶层首个分隔符位置（忽略括号与引号内部）。
fn find_top_level(text: &str, separator: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            }
            else if ch == '\\' {
                escaped = true;
            }
            else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 && ch == separator => return Some(index),
            _ => {}
        }
    }
    None
}

fn broken_wildcard_arm(source: &str, arm: &MatchArm) -> bool {
    matches!(arm.pattern, oak_valkyrie::ast::Pattern::Wildcard(_))
        && source.get(arm.span.start..arm.span.end).is_some_and(|text| text.trim_start().starts_with(':'))
}

/// 从 match 表达式 span 恢复 pattern 与 Oak body 表达式。
pub fn recover_match_arms(source: &str, match_span: Range<usize>) -> Result<Vec<(HirPattern, TermExpression)>, HirError> {
    let span = DiagnosticSpan { start: match_span.start, end: match_span.end };
    let interior = extract_match_block_interior(source, match_span.clone())
        .ok_or_else(|| HirError::match_recovery_failed(MatchRecoveryStage::BlockInterior, None, span.clone()))?;
    let raw_arms = split_match_arms(&interior);
    if raw_arms.is_empty() {
        return Err(HirError::match_recovery_failed(MatchRecoveryStage::Arms, None, span));
    }
    raw_arms
        .into_iter()
        .map(|(pattern_text, body_text)| {
            let span = DiagnosticSpan { start: match_span.start, end: match_span.end };
            let pattern = match parse_recovered_pattern(&pattern_text) {
                Some(pattern) => pattern,
                None => {
                    if let Some(error) = invalid_range_pattern_error(&pattern_text, span.clone()) {
                        return Err(error);
                    }
                    return Err(HirError::match_recovery_failed(MatchRecoveryStage::Pattern, Some(pattern_text.clone()), span.clone()));
                }
            };
            let body = parse_arm_body_expression(&body_text, span)?;
            Ok((pattern, body))
        })
        .collect()
}

fn count_match_arms_in_source(source: &str, match_span: Range<usize>) -> usize {
    extract_match_block_interior(source, match_span).map(|interior| split_match_arms(&interior).len()).unwrap_or(0)
}

fn extract_match_block_interior(source: &str, match_span: Range<usize>) -> Option<String> {
    let text = source.get(match_span)?;
    let open = text.find('{')?;
    let mut depth = 0i32;
    let mut close = None;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    Some(text[open + 1..close].to_string())
}

fn split_match_arms(interior: &str) -> Vec<(String, String)> {
    let mut arms = Vec::new();
    let mut index = 0usize;
    while index < interior.len() {
        index = skip_ascii_whitespace(interior, index);
        if index >= interior.len() {
            break;
        }
        if let Some(rest) = interior.get(index..) {
            if rest.starts_with("case ") {
                index += 5;
                index = skip_ascii_whitespace(interior, index);
                let pattern_end = find_pattern_colon(interior, index);
                if pattern_end <= index {
                    index += 1;
                    continue;
                }
                let pattern = interior[index..pattern_end].trim().to_string();
                index = pattern_end + 1;
                index = skip_ascii_whitespace(interior, index);
                let (body, next) = read_arm_body(interior, index);
                arms.push((pattern, body));
                index = next;
                continue;
            }
            if rest.starts_with("else") && is_word_boundary(rest, 4) {
                index += 4;
                index = skip_ascii_whitespace(interior, index);
                if interior.as_bytes().get(index) == Some(&b':') {
                    index += 1;
                }
                index = skip_ascii_whitespace(interior, index);
                let (body, next) = read_arm_body(interior, index);
                arms.push(("else".to_string(), body));
                index = next;
                continue;
            }
            if let Some(consumed) = default_arm_marker(rest) {
                index += consumed;
                index = skip_ascii_whitespace(interior, index);
                let (body, next) = read_arm_body(interior, index);
                arms.push(("else".to_string(), body));
                index = next;
                continue;
            }
        }
        index += 1;
    }
    arms
}

fn is_word_boundary(rest: &str, keyword_len: usize) -> bool {
    rest[keyword_len..].chars().next().is_none_or(|ch| ch.is_whitespace() || ch == ':')
}

fn skip_ascii_whitespace(text: &str, start: usize) -> usize {
    let mut index = start;
    while index < text.len() && text.as_bytes()[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn find_pattern_colon(text: &str, start: usize) -> usize {
    let mut depth = 0i32;
    let bytes = text.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b':' if depth == 0 => return index,
            _ => {}
        }
        index += 1;
    }
    start
}

fn read_arm_body(text: &str, start: usize) -> (String, usize) {
    let mut index = start;
    let body_start = index;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    while index < bytes.len() {
        if depth == 0 && starts_next_arm(text, index) {
            break;
        }
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    (text[body_start..index].trim().to_string(), index)
}

fn starts_next_arm(text: &str, index: usize) -> bool {
    let Some(rest) = text.get(index..)
    else {
        return false;
    };
    if rest.starts_with("case ") {
        return true;
    }
    if rest.starts_with("else") && is_word_boundary(rest, 4) {
        return true;
    }
    if default_arm_marker(rest).is_some() {
        return true;
    }
    false
}

/// `default` 仅在紧随（可隔空白）`:` 时才是臂标记，与解析器同一规则；
/// 否则它是臂体内的普通标识符表达式（如读取名为 `default` 的绑定）。
/// 返回连冒号在内的消费长度。
fn default_arm_marker(rest: &str) -> Option<usize> {
    if !(rest.starts_with("default") && is_word_boundary(rest, 7)) {
        return None;
    }
    let mut index = 7;
    while rest.as_bytes().get(index).is_some_and(|byte| byte.is_ascii_whitespace()) {
        index += 1;
    }
    if rest.as_bytes().get(index) == Some(&b':') { Some(index + 1) } else { None }
}

/// 从源码切分得到的区间模式文本恢复区间模式。
///
/// 端点契约与整体恢复一致：只接受整数字面量。非区间文本返回 `None`。
pub fn range_pattern_from_text(text: &str) -> Option<HirPattern> {
    let (start, end, inclusive_end) = parse_range_pattern(text)?;
    let start = parse_range_endpoint(start)?;
    let end = parse_range_endpoint(end)?;
    Some(HirPattern::Range { start, end, inclusive_end })
}

fn parse_recovered_pattern(text: &str) -> Option<HirPattern> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text == "_" {
        return Some(HirPattern::Wildcard);
    }
    if text == "else" || text == "default" {
        return Some(HirPattern::Else);
    }
    if let Some((start, end, inclusive_end)) = parse_range_pattern(text) {
        // 区间端点与 HIR lowering 同一契约：只接受整数形态，否则整体恢复失败。
        let start = parse_range_endpoint(start)?;
        let end = parse_range_endpoint(end)?;
        return Some(HirPattern::Range { start, end, inclusive_end });
    }
    parse_hir_literal(text).map(HirPattern::Literal)
}

fn parse_range_endpoint(text: Option<&str>) -> Option<Option<HirLiteral>> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty())
    else {
        return Some(None);
    };
    match parse_hir_literal(text) {
        Some(literal @ HirLiteral::Integer64(_)) => Some(Some(literal)),
        _ => None,
    }
}

/// 区间模式端点不合法时给出结构化诊断。
///
/// Oak 的 `Pattern` 没有区间变体，区间只能在源码恢复阶段识别；端点必须按整数字面量
/// 解释，`2..=8.5`、`true..=8` 等种类不一致或非整数端点在这里拒绝。
fn invalid_range_pattern_error(pattern_text: &str, span: DiagnosticSpan) -> Option<HirError> {
    let (start, end, _) = parse_range_pattern(pattern_text)?;
    let endpoint = [start, end]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|text| !text.is_empty() && !matches!(parse_hir_literal(text), Some(HirLiteral::Integer64(_))))?;
    Some(HirError::InvalidRangePattern {
        pattern: pattern_text.to_string(),
        reason: format!("endpoint `{endpoint}` is not an integer literal"),
        source: nyar_types::DiagnosticSource { id: "<hir_lowering>".into(), revision: None },
        span,
    })
}

fn parse_range_pattern(text: &str) -> Option<(Option<&str>, Option<&str>, bool)> {
    if let Some((start, end)) = text.split_once("..=") {
        return Some((Some(start.trim()), Some(end.trim()), true));
    }
    if let Some((start, end)) = text.split_once("..") {
        return Some((Some(start.trim()), Some(end.trim()), false));
    }
    None
}

fn parse_hir_literal(text: &str) -> Option<HirLiteral> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text == "true" || text == "false" {
        return Some(HirLiteral::Bool(text == "true"));
    }
    // 单引号捕获（`char`/`u8`/raw 文本）与表达式侧共用同一解码合同；
    // 恢复路径的 span 仅作兜底，解码失败由上层归入 `MatchRecoveryStage::Pattern`。
    if let Some((prefix, content, content_start)) =
        super::single_quote_literal::split_single_quote_capture(text, 0)
    {
        let prefix_name = prefix.as_ref().map(|(name, _)| *name);
        let decoded =
            super::single_quote_literal::decode_single_quote_literal(prefix, content, &(0..text.len()), content_start)
                .ok()?;
        return Some(super::single_quote_literal::single_quote_literal_value(decoded, prefix_name));
    }
    if let Ok(value) = text.parse::<i64>() {
        return Some(HirLiteral::Integer64(value));
    }
    if let Ok(value) = text.parse::<f64>() {
        return Some(HirLiteral::Float64(OrderedFloat(value)));
    }
    None
}

fn parse_arm_body_expression(body: &str, span: DiagnosticSpan) -> Result<TermExpression, HirError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(HirError::match_recovery_failed(MatchRecoveryStage::ArmBody, None, span));
    }
    let wrapper = format!("micro __match_arm_body() {{ {trimmed} }}");
    // 包装文本是合成源：诊断身份明确落 `<frontend:match-arm-body>`；
    // 臂体在来源文件中的站点由 `MatchRecoveryFailed` 的 span 承担。
    let root = parse_source_with_identity(&wrapper, "<frontend:match-arm-body>")?;
    let micro = root
        .items
        .iter()
        .find_map(|item| match item {
            StatementNode::Micro(micro) if micro.name.name == "__match_arm_body" => Some(micro),
            _ => None,
        })
        .ok_or_else(|| HirError::match_recovery_failed(MatchRecoveryStage::ArmBodyExpression, None, span.clone()))?;
    micro
        .body
        .statements
        .last()
        .and_then(|statement| match statement {
            oak_valkyrie::ast::Statement::Expression(expr_stmt) if !expr_stmt.semi => Some(expr_stmt.expr.clone()),
            _ => None,
        })
        .ok_or_else(|| HirError::match_recovery_failed(MatchRecoveryStage::ArmBodyExpression, None, span))
}
