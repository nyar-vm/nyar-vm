//! 单引号捕获字面量的语义判定：字符 / 字节 / raw 字符串。
//!
//! 词法只按等量引号串捕获原始文本（Oak `SingleQuoteLiteral` 节点携带未解码内容与
//! 前缀标识符）；是否解释转义、判定为 `char`/`u8`/字符串，全部在本模块完成：
//!
//! - 无前缀：cooked，解码 `\n \t \r \0 \\ \' \" \u{...}`；解码后恰好一个字符归
//!   `char`（u16 code unit，超出 BMP 结构化失败），多个字符归 `Utf8` 字符串。
//! - `r` 前缀：raw，不解码任何转义；长度判定同上。
//! - `b` 前缀：字节字面量，解码 `\n \t \r \0 \\ \' \" \xNN`；必须恰好一个字节，
//!   裸字符限 ASCII。
//!
//! 凡"转义一个与定界符相同的引号"在捕获阶段已不可能出现（捕获在第一个等量引号串
//! 处终止），本模块不再为此保留兼容分支。

use std::ops::Range;

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    syntax::oak::ast::Identifier as OakIdentifier,
    types::{
        Identifier,
        hir::{HirExprKind, HirLiteral, HirStringLiteral, HirStringSegment},
    },
};

use super::errors::{HirError, SingleQuoteLiteralFailure};

/// 单引号捕获解码后的语义归属。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedSingleQuoteLiteral {
    /// 恰好一个字符：`char`（u16 code unit）。
    Character(u16),
    /// 恰好一个字节：`u8`（仅 `b` 前缀）。
    Byte(u8),
    /// 多个字符（或空捕获）：`Utf8` 字符串。
    Text(String),
}

/// 判定并解码单引号捕获字面量；失败一律结构化并携带精确 span。
///
/// `prefix` 是紧邻引号串前的标识符名与源码区间（无前缀为 `None`）；
/// `literal_span` 是整个字面量（含前缀与两侧引号串）的源码区间；
/// `content_start` 是内容首字节的源码偏移，用于把转义错误定位到具体序列。
pub(crate) fn decode_single_quote_literal(
    prefix: Option<(&str, Range<usize>)>,
    content: &str,
    literal_span: &Range<usize>,
    content_start: usize,
) -> Result<DecodedSingleQuoteLiteral, HirError> {
    match prefix {
        Some(("r", _)) => classify_chars(content.chars().collect(), literal_span),
        Some(("b", _)) => decode_byte_literal(content, literal_span, content_start),
        Some((name, span)) => {
            let prefix_span = DiagnosticSpan { start: span.start, end: span.end };
            Err(single_quote_literal_error(SingleQuoteLiteralFailure::UnknownPrefix { prefix: name.to_string() }, prefix_span))
        }
        None => {
            let decoded = decode_cooked_chars(content, content_start)?;
            classify_chars(decoded, literal_span)
        }
    }
}

/// 从原始捕获文本（含前缀与两侧引号串）拆出结构化解码输入。
///
/// 模式位的 Oak 字面量节点只携带源码原文；返回 `None` 表示文本不是单引号捕获
/// 形态（数字、双引号字符串等），调用方按其他字面量门类继续判定。
pub(crate) fn split_single_quote_capture(raw: &str, literal_start: usize) -> Option<(Option<(&str, Range<usize>)>, &str, usize)> {
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
        if prefix_text.is_empty() { None } else { Some((prefix_text, literal_start..literal_start + quote_index)) };
    let content_start = literal_start + quote_index + quote_count;
    Some((prefix, content, content_start))
}

fn is_valid_prefix_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(characters.next(), Some(first) if first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

/// 把字符序列归类为字符或字符串：恰好一个字符归 `char`，其余（含空捕获）归 `Utf8`。
fn classify_chars(decoded: Vec<char>, literal_span: &Range<usize>) -> Result<DecodedSingleQuoteLiteral, HirError> {
    match decoded.as_slice() {
        [] => Ok(DecodedSingleQuoteLiteral::Text(String::new())),
        [single] => {
            let value = *single as u32;
            if value > 0xFFFF {
                let span = DiagnosticSpan { start: literal_span.start, end: literal_span.end };
                return Err(single_quote_literal_error(SingleQuoteLiteralFailure::CharacterBeyondBmp { value }, span));
            }
            Ok(DecodedSingleQuoteLiteral::Character(value as u16))
        }
        _ => Ok(DecodedSingleQuoteLiteral::Text(decoded.into_iter().collect())),
    }
}

fn single_quote_literal_error(failure: SingleQuoteLiteralFailure, span: DiagnosticSpan) -> HirError {
    HirError::InvalidSingleQuoteLiteral { failure, source: DiagnosticSource { id: "<hir_lowering>".into(), revision: None }, span }
}

fn escape_span(content_start: usize, start: usize, end: usize) -> DiagnosticSpan {
    DiagnosticSpan { start: content_start + start, end: content_start + end }
}

/// 解码 cooked 字符内容（`\n \t \r \0 \\ \' \" \u{...}`）。
fn decode_cooked_chars(content: &str, content_start: usize) -> Result<Vec<char>, HirError> {
    let mut decoded = Vec::new();
    let mut chars = content.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let Some((_, escaped)) = chars.next()
        else {
            return Err(single_quote_literal_error(
                SingleQuoteLiteralFailure::DanglingEscape,
                escape_span(content_start, index, content.len()),
            ));
        };
        match escaped {
            'n' => decoded.push('\n'),
            't' => decoded.push('\t'),
            'r' => decoded.push('\r'),
            '0' => decoded.push('\0'),
            '\\' => decoded.push('\\'),
            '\'' => decoded.push('\''),
            '"' => decoded.push('"'),
            'u' => {
                let (code_point, end) = parse_unicode_escape(content, index, content_start)?;
                decoded.push(code_point);
                // 消费 `\u{...}` 主体（`\` 与 `u` 已由迭代器读出）。
                while chars.peek().is_some_and(|(next_index, _)| *next_index < end) {
                    chars.next();
                }
            }
            _ => {
                let escape_text: String = content[index..=index + escaped.len_utf8()].to_string();
                return Err(single_quote_literal_error(
                    SingleQuoteLiteralFailure::InvalidEscapeSequence { escape: escape_text },
                    escape_span(content_start, index, index + 1 + escaped.len_utf8()),
                ));
            }
        }
    }
    Ok(decoded)
}

/// 解析 `\u{...}`：返回解码字符与 `}` 之后的字节下标。
fn parse_unicode_escape(content: &str, backslash_index: usize, content_start: usize) -> Result<(char, usize), HirError> {
    // `\` 与 `u` 已经读出；当前应位于 `{`。
    let open_index = backslash_index + 2;
    let invalid = |end: usize| {
        let escape_text: String = content[backslash_index..end].to_string();
        single_quote_literal_error(
            SingleQuoteLiteralFailure::InvalidUnicodeEscape { escape: escape_text },
            escape_span(content_start, backslash_index, end),
        )
    };
    let bytes = content.as_bytes();
    if bytes.get(open_index) != Some(&b'{') {
        return Err(invalid(open_index.min(content.len())));
    }
    let mut digits_end = open_index + 1;
    while digits_end < content.len() && bytes[digits_end] != b'}' {
        digits_end += 1;
    }
    if digits_end >= content.len() {
        return Err(invalid(content.len()));
    }
    let digits = &content[open_index + 1..digits_end];
    let Some(value) = u32::from_str_radix(digits, 16).ok().filter(|value| *value <= 0x10FFFF)
    else {
        return Err(invalid(digits_end + 1));
    };
    let Some(character) = char::from_u32(value)
    else {
        return Err(invalid(digits_end + 1));
    };
    Ok((character, digits_end + 1))
}

/// 解码字节字面量（`\n \t \r \0 \\ \' \" \xNN`）；必须恰好一个字节。
fn decode_byte_literal(content: &str, literal_span: &Range<usize>, content_start: usize) -> Result<DecodedSingleQuoteLiteral, HirError> {
    let literal_diagnostic_span = DiagnosticSpan { start: literal_span.start, end: literal_span.end };
    if content.is_empty() {
        return Err(single_quote_literal_error(SingleQuoteLiteralFailure::EmptyByteLiteral, literal_diagnostic_span));
    }
    let mut bytes_out = Vec::new();
    let mut chars = content.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if character != '\\' {
            if !character.is_ascii() {
                let character_span = escape_span(content_start, index, index + character.len_utf8());
                return Err(single_quote_literal_error(
                    SingleQuoteLiteralFailure::ByteLiteralNotAscii { character: character.to_string() },
                    character_span,
                ));
            }
            bytes_out.push(character as u8);
            continue;
        }
        let Some((_, escaped)) = chars.next()
        else {
            return Err(single_quote_literal_error(
                SingleQuoteLiteralFailure::DanglingEscape,
                escape_span(content_start, index, content.len()),
            ));
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
                    let escape_text: String = content[index..hex_end.min(content.len())].to_string();
                    return Err(single_quote_literal_error(
                        SingleQuoteLiteralFailure::InvalidEscapeSequence { escape: escape_text },
                        escape_span(content_start, index, hex_end.min(content.len())),
                    ));
                };
                bytes_out.push(value);
                // 消费两个十六进制位。
                chars.next();
                chars.next();
            }
            'u' => {
                return Err(single_quote_literal_error(
                    SingleQuoteLiteralFailure::UnicodeEscapeInByteLiteral,
                    escape_span(content_start, index, index + 2),
                ));
            }
            _ => {
                let escape_text: String = content[index..=index + escaped.len_utf8()].to_string();
                return Err(single_quote_literal_error(
                    SingleQuoteLiteralFailure::InvalidEscapeSequence { escape: escape_text },
                    escape_span(content_start, index, index + 1 + escaped.len_utf8()),
                ));
            }
        }
    }
    match bytes_out.as_slice() {
        [single] => Ok(DecodedSingleQuoteLiteral::Byte(*single)),
        _ => Err(single_quote_literal_error(
            SingleQuoteLiteralFailure::ByteLiteralNotSingleByte { count: bytes_out.len() },
            literal_diagnostic_span,
        )),
    }
}

/// 把解码结果落到 HIR 字面量；`prefix` 原样携带，不新造语义标记。
pub(crate) fn single_quote_literal_value(decoded: DecodedSingleQuoteLiteral, prefix: Option<&str>) -> HirLiteral {
    match decoded {
        DecodedSingleQuoteLiteral::Character(value) => HirLiteral::Character(value),
        DecodedSingleQuoteLiteral::Byte(value) => HirLiteral::Byte(value),
        DecodedSingleQuoteLiteral::Text(text) => HirLiteral::String(HirStringLiteral {
            prefix: prefix.map(Identifier::new),
            quote_count: 1,
            segments: vec![HirStringSegment::Text(text)],
        }),
    }
}

/// 把解码结果落到 HIR 表达式节点；`prefix` 原样携带，不新造语义标记。
pub(crate) fn single_quote_literal_kind(decoded: DecodedSingleQuoteLiteral, prefix: Option<&OakIdentifier>) -> HirExprKind {
    HirExprKind::Literal(single_quote_literal_value(decoded, prefix.map(|prefix| prefix.name.as_str())))
}
