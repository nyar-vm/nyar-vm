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
//! 引号种类与定界长度不改变捕获后的处理管线。归属遵循 §4.10 B：只有单引号且
//! 解码结果恰好为一个 BMP 字符时按 `char` 默认落 `Character`；其余捕获保持未定型
//! 文本形态，由上下文约束求解定型，确无上下文时才按引号默认（单→`char`、
//! 双→`utf8`）在最终消费点结算，不提供长度回退。

use std::ops::Range;

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    syntax::oak::{
        ast::{Identifier as OakIdentifier, QuoteKind, QuoteLiteral, StringSegment as OakSegment, TermExpression},
        parse_quote_template,
    },
    types::{
        Identifier,
        hir::{HirLiteral, HirQuoteKind, HirStringLiteral, HirStringSegment},
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

/// 引号捕获解释后的值形态。
#[derive(Debug)]
pub(crate) enum DecodedQuoteCapture {
    /// 单引号、无插值、解码后恰好一个 BMP 字符。
    Character(u16),
    /// `b` 前缀字节字面量。
    Byte(u8),
    /// 无插值文本（已解码）。
    PlainText(String),
    /// 含插值的模板；文本段已解码。
    Template(Vec<QuoteTemplatePart>),
}

/// 解释 Oak 引号捕获；`allow_interpolation` 为 false 时常量位（模式等）拒绝插值段。
pub(crate) fn decode_quote_capture(literal: &QuoteLiteral, allow_interpolation: bool) -> Result<DecodedQuoteCapture, HirError> {
    match literal.prefix.as_ref().map(|prefix| prefix.name.as_str()) {
        Some("r") => Ok(classify(literal, literal.content.clone())),
        Some("b") => decode_byte_literal(literal).map(DecodedQuoteCapture::Byte),
        Some(name) => {
            let span = literal.prefix.as_ref().map_or_else(|| literal.span.start..literal.span.end, |prefix| prefix.span.start..prefix.span.end);
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
    let prefix =
        if prefix_text.is_empty() { None } else { Some(OakIdentifier { name: prefix_text.to_string(), span: oak_core::Range { start: literal_start, end: literal_start + quote_index } }) };
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

/// 把模式位的解码结果落到 HIR 字面量；插值段在常量位已被拒绝。
pub(crate) fn pattern_literal_value(decoded: DecodedQuoteCapture, prefix: Option<&str>, quote: QuoteKind, quote_count: u8) -> Result<HirLiteral, HirError> {
    match decoded {
        DecodedQuoteCapture::Character(value) => Ok(HirLiteral::Character(value)),
        DecodedQuoteCapture::Byte(value) => Ok(HirLiteral::Byte(value)),
        DecodedQuoteCapture::PlainText(text) => Ok(HirLiteral::String(HirStringLiteral {
            prefix: prefix.map(Identifier::new),
            quote_count,
            quote: hir_quote_kind(quote),
            segments: vec![HirStringSegment::Text(text)],
        })),
        DecodedQuoteCapture::Template(parts) => {
            let span = parts
                .iter()
                .find_map(|part| match part {
                    QuoteTemplatePart::Interpolation { span, .. } => Some(span.clone()),
                    _ => None,
                })
                .unwrap_or(0..0);
            Err(quote_literal_error(QuoteLiteralFailure::TemplateSyntax { message: "interpolation is not a constant pattern".to_string() }, span))
        }
    }
}

/// cooked 捕获：模板结构解析 + 文本段转义解码。
fn decode_cooked_capture(literal: &QuoteLiteral, allow_interpolation: bool) -> Result<DecodedQuoteCapture, HirError> {
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

    if has_interpolation { Ok(DecodedQuoteCapture::Template(parts)) } else { Ok(classify(literal, plain)) }
}

/// 归属：单引号且解码结果恰好为一个 BMP 字符时按 `char` 默认落 `Character`；
/// 其余捕获（空、多字符、超 BMP、双引号）一律保持未定型文本捕获，由上下文约束
/// 求解或最终默认决定（§4.10 B），不按长度回退、不提前报表示拒绝。
fn classify(literal: &QuoteLiteral, decoded: String) -> DecodedQuoteCapture {
    if literal.quote == QuoteKind::Single {
        let mut characters = decoded.chars();
        if let (Some(single), None) = (characters.next(), characters.next())
            && let Ok(value) = u16::try_from(u32::from(single))
        {
            return DecodedQuoteCapture::Character(value);
        }
    }
    DecodedQuoteCapture::PlainText(decoded)
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
                return Err(quote_literal_error(QuoteLiteralFailure::InvalidEscapeSequence { escape }, base + index..base + index + 1 + escaped.len_utf8()));
            }
        }
    }
    Ok(decoded)
}

/// 解析 `\uXXXX`：恰好四位十六进制，且必须是合法 Unicode 标量（代理位拒绝）。
/// 返回解码字符与十六进制位之后的字节下标。
fn parse_unicode_escape(text: &str, backslash_index: usize, base: usize) -> Result<(char, usize), HirError> {
    let digits_start = backslash_index + 2;
    let digits_end = digits_start + 4;
    let invalid = |end: usize| {
        let escape = text[backslash_index..end.min(text.len())].to_string();
        quote_literal_error(QuoteLiteralFailure::InvalidUnicodeEscape { escape }, base + backslash_index..base + end.min(text.len()))
    };
    let digits = text.get(digits_start..digits_end).ok_or_else(|| invalid(digits_end))?;
    let value = u32::from_str_radix(digits, 16).map_err(|_| invalid(digits_end))?;
    let character = char::from_u32(value).ok_or_else(|| invalid(digits_end))?;
    Ok((character, digits_end))
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
                let hex_start = index + 2;
                let hex_end = hex_start + 2;
                let value = content.get(hex_start..hex_end).and_then(|digits| u8::from_str_radix(digits, 16).ok());
                let Some(value) = value
                else {
                    let escape = content[index..hex_end.min(content.len())].to_string();
                    return Err(quote_literal_error(
                        QuoteLiteralFailure::InvalidEscapeSequence { escape },
                        escape_span(content_start, index, hex_end.min(content.len())),
                    ));
                };
                bytes_out.push(value);
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
