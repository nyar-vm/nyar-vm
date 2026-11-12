//! 引号捕获字面量的统一语义判定：前缀解释、转义解码、模板绑定与 char/文本归属。
//!
//! 词法只按等量引号串捕获原始文本（Oak `QuoteLiteral` 节点携带未解码内容、
//! 引号种类、定界长度与紧邻前缀）；本模块是单双引号共用的唯一解释入口：
//!
//! - 无前缀：cooked。请求 Oak 对捕获区间做有边界模板解析，文本段按标准转义集
//!   （`\" \\ \/ \b \f \n \r \t \uXXXX` 与 `\'`、`\{`、`\}`）解码；未知/残缺转义
//!   与模板语法错误一律结构化报错并带原始源区间。
//! - `r` 前缀：raw，不解码转义、不解析插值。
//! - `b` 前缀：字节字面量，独立转义集（`\n \t \r \0 \\ \' \" \xNN`），必须恰好一个字节。
//! - 其它前缀：结构化报错，不按默认字符串成功。
//!
//! 引号种类与定界长度不改变捕获后的处理管线。捕获解码后保持未定型形态：单双引号
//! 进入同一字面量约束求解（§4.10 B），由 `quote_settle` 在 HIR 内唯一定型——注解、
//! 形参、返回、字段、容器元素与泛型约束先行联合求解，引号默认（单→`char`、
//! 双→`utf8`）最后且只用一次；无约束 `''`/`'ab'` 默认 char 后报表示错误，不提供长度回退。

use std::ops::Range;

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    syntax::oak::{
        ast::{Identifier as OakIdentifier, QuoteKind, QuoteLiteral, StringSegment as OakSegment, TermExpression},
        parse_quote_template,
    },
    types::{
        Identifier,
        hir::{HirLiteral, HirQuoteKind, HirStringLiteral, HirStringSegment, HirTextEncoding},
    },
};

use super::errors::{HirError, QuoteLiteralFailure};

/// Oak 引号种类映射为 HIR 语法元数据。
pub(crate) fn hir_quote_kind(quote: QuoteKind) -> HirQuoteKind {
    match quote {
        QuoteKind::Single => HirQuoteKind::Single,
        QuoteKind::Double => HirQuoteKind::Double,
    }
}

/// 模板片段：解码文本或待绑定插值。
#[derive(Debug)]
pub(crate) enum QuoteTemplatePart {
    /// 解码后的文本段。
    Text(String),
    /// 插值段；表达式保持 Oak AST，由 lowering 绑定为 HIR 并保留求值顺序。
    Interpolation {
        /// Oak 真实表达式树。
        expr: TermExpression,
        /// 是否为 locale（Fluent）插值。
        is_fluent: bool,
        /// `{ ... }` 区间的源码位置。
        span: Range<usize>,
    },
}

/// 引号捕获解释后的值形态。捕获保持未定型：字符/文本归属由 `quote_settle` 按上下文
/// 约束或引号默认结算（§4.10 B），decode 不再提前构造 `Character`。
#[derive(Debug)]
pub(crate) enum DecodedQuoteCapture {
    /// 退役变体：定型已移交 `quote_settle`，本 crate 内无构造点。
    /// 边界外的 `legion::workspace::planner::ast_attributes` 仍显式匹配它，待其 owner
    /// 删除该 arm 后随之下线；`PlainText` 已覆盖原语义。
    Character(u32),
    /// `b` 前缀字节字面量。
    Byte(u8),
    /// 无插值文本（已解码，未定型）。
    PlainText(String),
    /// 含插值的模板；文本段已解码。
    Template(Vec<QuoteTemplatePart>),
}

/// 未定型捕获可结算成的最终形态（§4.10 B 唯一绑定规则的输出）。
#[derive(Debug, Clone, Copy)]
pub(crate) enum SettledCapture {
    /// 恰好一个码位的 `char`（§4.12 字符合同：u32 码位，非 BMP 合法）。
    Char(u32),
    /// 编码文本；载体由上下文或引号默认决定。
    Text(HirTextEncoding),
}

/// 目标类型对未定型捕获呈现的约束族。分类唯一入口是
/// `TypeRelationContext::capture_target_kind`，各消费点不得另立规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureTargetKind {
    /// `char` 身份目标：接受恰好一个码位（两种引号均可）。
    Char,
    /// 编码文本目标：接受任意捕获（含空与多码位）。
    Text(HirTextEncoding),
    /// 数值目标：只接受 char 自然型（单引号）且恰好一个码位。
    Numeric,
    /// 其它目标：捕获不绑定。
    Other,
}

/// 唯一绑定规则（§4.10 B）：给定未定型捕获与目标约束族，判定能否绑定及最终形态。
/// 引号与码位数只在此处参与判定，禁止在下游按引号/长度为候选打分。
pub(crate) fn capture_binds(capture: &HirStringLiteral, target: CaptureTargetKind) -> Option<SettledCapture> {
    match target {
        CaptureTargetKind::Char => capture.as_single_char().map(SettledCapture::Char),
        CaptureTargetKind::Numeric => {
            if capture.quote == HirQuoteKind::Single {
                capture.as_single_char().map(SettledCapture::Char)
            }
            else {
                None
            }
        }
        CaptureTargetKind::Text(encoding) => Some(SettledCapture::Text(encoding)),
        CaptureTargetKind::Other => None,
    }
}

/// 引号默认（最后、只用一次）：纯文本单引号默认 `char`（非单码位返回 `None`，由调用方
/// 报表示错误，不提供长度回退）；双引号与动态模板默认 `utf8` 文本（§4.10 E）。
pub(crate) fn capture_quote_default(capture: &HirStringLiteral) -> Option<SettledCapture> {
    if !capture.is_plain_text() {
        return Some(SettledCapture::Text(HirTextEncoding::Utf8));
    }
    match capture.quote {
        HirQuoteKind::Single => capture.as_single_char().map(SettledCapture::Char),
        HirQuoteKind::Double => Some(SettledCapture::Text(HirTextEncoding::Utf8)),
    }
}

/// 纯文本捕获的码位数；含插值返回 `None`（表示错误文案用）。
pub(crate) fn capture_code_points(capture: &HirStringLiteral) -> Option<usize> {
    if !capture.is_plain_text() {
        return None;
    }
    Some(
        capture
            .segments
            .iter()
            .map(|segment| match segment {
                HirStringSegment::Text(text) => text.chars().count(),
                HirStringSegment::Interpolation { .. } => 0,
            })
            .sum(),
    )
}

/// 解释 Oak 引号捕获；`allow_interpolation` 为 false 时常量位（模式等）拒绝插值段。
pub(crate) fn decode_quote_capture(literal: &QuoteLiteral, allow_interpolation: bool) -> Result<DecodedQuoteCapture, HirError> {
    match literal.prefix.as_ref().map(|prefix| prefix.name.as_str()) {
        Some("r") => Ok(DecodedQuoteCapture::PlainText(literal.content.clone())),
        Some("b") => decode_byte_literal(literal).map(DecodedQuoteCapture::Byte),
        Some(name) => {
            let span =
                literal.prefix.as_ref().map_or_else(|| literal.span.start..literal.span.end, |prefix| prefix.span.start..prefix.span.end);
            Err(quote_literal_error(QuoteLiteralFailure::UnknownPrefix { prefix: name.to_string() }, span))
        }
        None => decode_cooked_capture(literal, allow_interpolation),
    }
}

/// 从模式位的原始捕获文本拆出 `QuoteLiteral`；不是单引号捕获形态时返回 `None`。
pub(crate) fn split_pattern_capture(raw: &str, literal_start: usize) -> Option<QuoteLiteral> {
    let quote_index = raw.find('\'')?;
    let prefix_text = &raw[..quote_index];
    if !prefix_text.is_empty() && !is_valid_prefix_identifier(prefix_text) {
        return None;
    }
    let after_prefix = &raw[quote_index..];
    let quote_count = after_prefix.chars().take_while(|character| *character == '\'').count();
    let body = &after_prefix[quote_count..];
    let closing: String = "'".repeat(quote_count);
    if body.len() < quote_count || !body.ends_with(&closing) {
        return None;
    }
    let content = &body[..body.len() - quote_count];
    let prefix = if prefix_text.is_empty() {
        None
    }
    else {
        Some(OakIdentifier { name: prefix_text.to_string(), span: oak_core::Range { start: literal_start, end: literal_start + quote_index } })
    };
    let content_start = literal_start + quote_index + quote_count;
    Some(QuoteLiteral {
        prefix,
        quote: QuoteKind::Single,
        quote_count: quote_count as u8,
        content: content.to_string(),
        content_span: oak_core::Range { start: content_start, end: content_start + content.len() },
        span: oak_core::Range { start: literal_start, end: literal_start + raw.len() },
    })
}

fn is_valid_prefix_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(characters.next(), Some(first) if first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

/// 从任意引号捕获的原始 token 文本（可带 `r`/`b` 前缀，单双引号与多引号均可）
/// 拆出 `QuoteLiteral`；不是完整捕获形态时返回 `None`。
fn split_quote_capture(raw: &str, literal_start: usize) -> Option<QuoteLiteral> {
    let text = raw.trim();
    let trim_offset = raw.len() - raw.trim_start().len();
    let quote_index = text.find(['\'', '"'])?;
    let prefix_text = &text[..quote_index];
    if !prefix_text.is_empty() && !is_valid_prefix_identifier(prefix_text) {
        return None;
    }
    let after_prefix = &text[quote_index..];
    let quote = match after_prefix.chars().next()? {
        '\'' => QuoteKind::Single,
        '"' => QuoteKind::Double,
        _ => return None,
    };
    let quote_char = after_prefix.chars().next()?;
    let quote_count = after_prefix.chars().take_while(|character| *character == quote_char).count();
    let body = &after_prefix[quote_count..];
    let closing: String = quote_char.to_string().repeat(quote_count);
    if body.len() < quote_count || !body.ends_with(&closing) {
        return None;
    }
    let content = &body[..body.len() - quote_count];
    let prefix = if prefix_text.is_empty() {
        None
    }
    else {
        Some(OakIdentifier {
            name: prefix_text.to_string(),
            span: oak_core::Range { start: literal_start + trim_offset, end: literal_start + trim_offset + quote_index },
        })
    };
    let content_start = literal_start + trim_offset + quote_index + quote_count;
    Some(QuoteLiteral {
        prefix,
        quote,
        quote_count: quote_count as u8,
        content: content.to_string(),
        content_span: oak_core::Range { start: content_start, end: content_start + content.len() },
        span: oak_core::Range { start: literal_start + trim_offset, end: literal_start + trim_offset + text.len() },
    })
}

/// 统一解码入口：把字符串 token 的原始文本（含定界与前缀）解成文本值。
///
/// 模板常量求值、架构匹配等非表达式入口必须消费本函数，不得只剥首尾引号；
/// 转义集、`\{`/`\}` 与未知/残缺转义诊断与表达式捕获完全同源。
pub(crate) fn decode_quoted_token(raw: &str, base: usize) -> Result<String, HirError> {
    let span = base..base + raw.len();
    let Some(literal) = split_quote_capture(raw, base)
    else {
        return Err(quote_literal_error(QuoteLiteralFailure::TemplateSyntax { message: "not a complete quoted capture".to_string() }, span));
    };
    match decode_quote_capture(&literal, false)? {
        DecodedQuoteCapture::PlainText(text) => Ok(text),
        DecodedQuoteCapture::Byte(_) => {
            Err(quote_literal_error(QuoteLiteralFailure::TemplateSyntax { message: "byte literal is not text".to_string() }, span))
        }
        DecodedQuoteCapture::Character(_) => {
            Err(quote_literal_error(QuoteLiteralFailure::TemplateSyntax { message: "character capture is not text".to_string() }, span))
        }
        DecodedQuoteCapture::Template(_) => {
            Err(quote_literal_error(QuoteLiteralFailure::TemplateSyntax { message: "interpolation is not a constant text".to_string() }, span))
        }
    }
}

/// 把文本编码成可被本模块解码回原文的引号捕获（合法定界 + 标准转义）。
///
/// 定界符只选内容首尾都不是的引号种类，内部同种引号用更长的引号 run 容纳；
/// 首尾同时被两种引号占用时无法用单个捕获表达，返回失败原因，由调用方带源区间报错。
pub(crate) fn encode_text_capture(text: &str) -> Result<String, String> {
    let delimiter = if !text.starts_with('"') && !text.ends_with('"') {
        '"'
    }
    else if !text.starts_with('\'') && !text.ends_with('\'') {
        '\''
    }
    else {
        return Err("text starts and ends with quote characters that no single capture can delimit".to_string());
    };
    let longest_run = text
        .chars()
        .fold((0usize, 0usize), |(longest, current), character| {
            if character == delimiter {
                let next = current + 1;
                (longest.max(next), next)
            }
            else {
                (longest, 0)
            }
        })
        .0;
    let quote_count = if longest_run == 0 { 1 } else { (longest_run + 1).max(3) };
    let mut body = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => body.push_str("\\\\"),
            '{' => body.push_str("\\{"),
            '}' => body.push_str("\\}"),
            '\n' => body.push_str("\\n"),
            '\r' => body.push_str("\\r"),
            '\t' => body.push_str("\\t"),
            '\u{8}' => body.push_str("\\b"),
            '\u{C}' => body.push_str("\\f"),
            other if other.is_control() => body.push_str(&format!("\\u{:04X}", other as u32)),
            other => body.push(other),
        }
    }
    let delimiter = delimiter.to_string().repeat(quote_count);
    Ok(format!("{delimiter}{body}{delimiter}"))
}

/// 把模式位的解码结果落到 HIR 字面量；插值段在常量位已被拒绝。捕获保持未定型，
/// 字符/文本归属由 `quote_settle` 按被检类型或引号默认结算。
pub(crate) fn pattern_literal_value(
    decoded: DecodedQuoteCapture,
    prefix: Option<&str>,
    quote: QuoteKind,
    quote_count: u8,
) -> Result<HirLiteral, HirError> {
    match decoded {
        DecodedQuoteCapture::Byte(value) => Ok(HirLiteral::Byte(value)),
        // 退役变体无构造点；透传仅保持穷尽匹配，待边界外 legion arm 清退后一并下线。
        DecodedQuoteCapture::Character(value) => Ok(HirLiteral::Character(value)),
        DecodedQuoteCapture::PlainText(text) => Ok(HirLiteral::String(HirStringLiteral {
            prefix: prefix.map(Identifier::new),
            quote_count,
            quote: hir_quote_kind(quote),
            segments: vec![HirStringSegment::Text(text)],
            text_encoding: None,
        })),
        DecodedQuoteCapture::Template(parts) => {
            let span = parts
                .iter()
                .find_map(|part| match part {
                    QuoteTemplatePart::Interpolation { span, .. } => Some(span.clone()),
                    _ => None,
                })
                .unwrap_or(0..0);
            Err(quote_literal_error(
                QuoteLiteralFailure::TemplateSyntax { message: "interpolation is not a constant pattern".to_string() },
                span,
            ))
        }
    }
}

/// cooked 捕获：模板结构解析 + 文本段转义解码。
fn decode_cooked_capture(literal: &QuoteLiteral, allow_interpolation: bool) -> Result<DecodedQuoteCapture, HirError> {
    // 插值是双引号字符串的能力（见语言文档 lexical-structure：普通双引号字符串支持转义与插值）。
    // 单引号捕获是字符/文本捕获，内容里的花括号是数据，不参与模板结构解析。
    if matches!(literal.quote, QuoteKind::Single) {
        let decoded = decode_text_segment(&literal.content, literal.content_span.start)?;
        return Ok(DecodedQuoteCapture::PlainText(decoded));
    }
    let segments = parse_quote_template(&literal.content, literal.content_span.start).map_err(|error| {
        let span = error.source_offset().map_or_else(|| literal.span.start..literal.span.end, |offset| offset..offset + 1);
        quote_literal_error(QuoteLiteralFailure::TemplateSyntax { message: error.to_string() }, span)
    })?;

    let mut parts: Vec<QuoteTemplatePart> = Vec::new();
    let mut plain = String::new();
    let mut has_interpolation = false;
    for segment in segments {
        match segment {
            OakSegment::Text(text) => {
                let decoded = decode_text_segment(&text.content, text.span.start)?;
                if !has_interpolation {
                    plain.push_str(&decoded);
                }
                parts.push(QuoteTemplatePart::Text(decoded));
            }
            OakSegment::Interpolation(interpolation) => {
                if !allow_interpolation {
                    return Err(quote_literal_error(
                        QuoteLiteralFailure::TemplateSyntax { message: "interpolation is not a constant pattern".to_string() },
                        interpolation.span.start..interpolation.span.end,
                    ));
                }
                has_interpolation = true;
                parts.push(QuoteTemplatePart::Interpolation {
                    expr: interpolation.expr,
                    is_fluent: interpolation.is_locale,
                    span: interpolation.span.start..interpolation.span.end,
                });
            }
        }
    }

    if has_interpolation { Ok(DecodedQuoteCapture::Template(parts)) } else { Ok(DecodedQuoteCapture::PlainText(plain)) }
}

/// 解码 cooked 文本段（标准转义集；未知/残缺一律结构化报错）。
fn decode_text_segment(text: &str, base: usize) -> Result<String, HirError> {
    let mut decoded = String::new();
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let Some((_, escaped)) = characters.next()
        else {
            return Err(quote_literal_error(QuoteLiteralFailure::DanglingEscape, base + index..base + text.len()));
        };
        match escaped {
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'b' => decoded.push('\u{8}'),
            'f' => decoded.push('\u{0C}'),
            '/' => decoded.push('/'),
            '\\' => decoded.push('\\'),
            '\'' => decoded.push('\''),
            '"' => decoded.push('"'),
            '{' => decoded.push('{'),
            '}' => decoded.push('}'),
            'u' => {
                let (value, digits_end) = parse_unicode_escape(text, index, base)?;
                decoded.push(value);
                while characters.peek().is_some_and(|(next_index, _)| *next_index < digits_end) {
                    characters.next();
                }
            }
            _ => {
                let escape = text[index..index + 1 + escaped.len_utf8()].to_string();
                return Err(quote_literal_error(
                    QuoteLiteralFailure::InvalidEscapeSequence { escape },
                    base + index..base + index + 1 + escaped.len_utf8(),
                ));
            }
        }
    }
    Ok(decoded)
}

/// 解析 `\uXXXX`：恰好四位十六进制，且必须是合法 Unicode 标量（代理位拒绝）。
/// 返回解码字符与十六进制位之后的字节下标。十六进制位按字符边界收集，诊断切片
/// 只落在字符边界上，非 ASCII 非法输入不得在 UTF-8 字符内部断开（S5 panic 边界）。
fn parse_unicode_escape(text: &str, backslash_index: usize, base: usize) -> Result<(char, usize), HirError> {
    let digits_start = backslash_index + 2;
    let rest = &text[digits_start..];
    // 收集至多四位十六进制 ASCII 数字；遇非十六进制或结尾即停。`end` 始终是字符边界。
    let mut digits = String::new();
    let mut end = digits_start;
    for (offset, character) in rest.char_indices() {
        if digits.len() == 4 || !character.is_ascii_hexdigit() {
            break;
        }
        digits.push(character);
        end = digits_start + offset + character.len_utf8();
    }
    let invalid = || {
        let escape = text[backslash_index..end].to_string();
        quote_literal_error(QuoteLiteralFailure::InvalidUnicodeEscape { escape }, base + backslash_index..base + end)
    };
    if digits.len() != 4 {
        return Err(invalid());
    }
    let value = u32::from_str_radix(&digits, 16).map_err(|_| invalid())?;
    let character = char::from_u32(value).ok_or_else(|| invalid())?;
    Ok((character, end))
}

/// 解码字节字面量（`\n \t \r \0 \\ \' \" \xNN`）；必须恰好一个字节。
fn decode_byte_literal(literal: &QuoteLiteral) -> Result<u8, HirError> {
    let content = literal.content.as_str();
    let literal_span = literal.span.start..literal.span.end;
    let content_start = literal.content_span.start;
    if content.is_empty() {
        return Err(quote_literal_error(QuoteLiteralFailure::EmptyByteLiteral, literal_span));
    }
    let mut bytes_out = Vec::new();
    let mut characters = content.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if character != '\\' {
            if !character.is_ascii() {
                let character_span = escape_span(content_start, index, index + character.len_utf8());
                return Err(quote_literal_error(QuoteLiteralFailure::ByteLiteralNotAscii { character: character.to_string() }, character_span));
            }
            bytes_out.push(character as u8);
            continue;
        }
        let Some((_, escaped)) = characters.next()
        else {
            return Err(quote_literal_error(QuoteLiteralFailure::DanglingEscape, escape_span(content_start, index, content.len())));
        };
        match escaped {
            'n' => bytes_out.push(b'\n'),
            't' => bytes_out.push(b'\t'),
            'r' => bytes_out.push(b'\r'),
            '0' => bytes_out.push(b'\0'),
            '\\' => bytes_out.push(b'\\'),
            '\'' => bytes_out.push(b'\''),
            '"' => bytes_out.push(b'"'),
            'x' => {
                // 恰好两位十六进制 ASCII 数字；按字符边界收集，诊断切片不得落在 UTF-8
                // 字符内部（与 `\u` 同一 panic 边界）。
                let hex_start = index + 2;
                let rest = &content[hex_start..];
                let mut digits = String::new();
                let mut consumed = 0;
                for digit in rest.chars() {
                    if digits.len() == 2 || !digit.is_ascii_hexdigit() {
                        break;
                    }
                    digits.push(digit);
                    consumed += digit.len_utf8();
                }
                if digits.len() != 2 {
                    let end = hex_start + consumed;
                    return Err(quote_literal_error(
                        QuoteLiteralFailure::InvalidEscapeSequence { escape: content[index..end].to_string() },
                        escape_span(content_start, index, end),
                    ));
                }
                bytes_out.push(u8::from_str_radix(&digits, 16).unwrap_or_else(|_| unreachable!("两位十六进制必可解析")));
                characters.next();
                characters.next();
            }
            'u' => {
                return Err(quote_literal_error(QuoteLiteralFailure::UnicodeEscapeInByteLiteral, escape_span(content_start, index, index + 2)));
            }
            _ => {
                let escape = content[index..=index + escaped.len_utf8()].to_string();
                return Err(quote_literal_error(
                    QuoteLiteralFailure::InvalidEscapeSequence { escape },
                    escape_span(content_start, index, index + 1 + escaped.len_utf8()),
                ));
            }
        }
    }
    match bytes_out.as_slice() {
        [single] => Ok(*single),
        _ => Err(quote_literal_error(QuoteLiteralFailure::ByteLiteralNotSingleByte { count: bytes_out.len() }, literal_span)),
    }
}

fn escape_span(content_start: usize, start: usize, end: usize) -> Range<usize> {
    content_start + start..content_start + end
}

fn quote_literal_error(failure: QuoteLiteralFailure, span: Range<usize>) -> HirError {
    HirError::InvalidQuoteLiteral {
        failure,
        source: DiagnosticSource { id: "<hir_lowering>".into(), revision: None },
        span: DiagnosticSpan { start: span.start, end: span.end },
    }
}

/// 结算阶段（`quote_settle`）的结构化表示错误：带捕获原文源区间，不静默降级。
pub(crate) fn settle_quote_error(failure: QuoteLiteralFailure, span: Range<usize>) -> HirError {
    quote_literal_error(failure, span)
}
