//! 图表标题 VX 字面量的往返测试。
//!
//! VX 没有反斜杠转义，字符串由引号 run 定界，`{` 开插值；这里逐条验证
//! `encode_v_text_literal` 产出的字面量能被前端解析、且 `Text` 段逐字还原原文。

use rusty_valkyrie::ValkyrieCompiler;
use rusty_valkyrie::asgard::report_build::encode_v_text_literal;

/// 编译一个字面量并返回 `(quote_count, 解码后的 Text 段)`。
fn literal_shape(source_literal: &str) -> Result<(usize, Vec<String>), String> {
    let source = format!("namespace probe;\nmicro f(): bool {{ let value = {source_literal}\nreturn true\n}}\n");
    let hir = ValkyrieCompiler::default().compile_source(&source).map_err(|error| error.to_string())?;
    let debug = format!("{hir:?}");
    let start = debug.find("String(HirStringLiteral").ok_or_else(|| format!("no string literal in {debug}"))?;
    if debug[start..].contains("Interpolation") {
        return Err(format!("literal interpolates instead of holding text: {}", &debug[start..]));
    }
    let quote_count = debug[start..]
        .strip_prefix("String(HirStringLiteral")
        .and_then(|rest| rest.trim_start().strip_prefix('{'))
        .and_then(|rest| rest.trim_start().strip_prefix("prefix: None, quote_count: "))
        .and_then(|rest| rest.split(',').next())
        .and_then(|digits| digits.trim().parse::<usize>().ok())
        .ok_or_else(|| format!("cannot read quote_count from {}", &debug[start..]))?;
    let segments_at = start + debug[start..].find("segments: [").ok_or("no segments")?;
    let body = &debug[segments_at + "segments: [".len()..];
    let end = body.find("] }").ok_or("unterminated segments")?;
    let mut texts = Vec::new();
    let mut rest = &body[..end];
    while let Some(at) = rest.find("Text(\"") {
        let content_start = at + "Text(\"".len();
        let (decoded, consumed) = decode_debug_str(&rest[content_start..])?;
        texts.push(decoded);
        rest = &rest[content_start + consumed..];
    }
    Ok((quote_count, texts))
}

/// 解码 Rust `Debug` 里的字符串内容（`\"`、`\\`、`\n`、`\t`、`\r`、`\u{..}`）。
fn decode_debug_str(raw: &str) -> Result<(String, usize), String> {
    let mut out = String::new();
    let mut chars = raw.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '"' => return Ok((out, index + 1)),
            '\\' => {
                let Some((_, escaped)) = chars.next() else { return Err("dangling escape".to_string()) };
                match escaped {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '0' => out.push('\0'),
                    'u' => {
                        let mut digits = String::new();
                        let mut closed = false;
                        for (_, digit) in chars.by_ref() {
                            match digit {
                                '{' => {}
                                '}' => {
                                    closed = true;
                                    break;
                                }
                                other => digits.push(other),
                            }
                        }
                        if !closed {
                            return Err("unterminated unicode escape".to_string());
                        }
                        let code = u32::from_str_radix(&digits, 16).map_err(|error| error.to_string())?;
                        out.push(char::from_u32(code).ok_or("invalid scalar value")?);
                    }
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
    }
    Err("unterminated string".to_string())
}

fn round_trip(text: &str) -> Result<(usize, String), String> {
    let literal = encode_v_text_literal(text).map_err(|error| error.to_string())?;
    let (quote_count, segments) = literal_shape(&literal)?;
    Ok((quote_count, segments.concat()))
}

#[test]
fn title_literal_round_trips_quotes_braces_and_backslashes() {
    let cases: Vec<&str> = vec![
        "plain title",
        "He said \"hi\" loudly",
        "He said \"\"hi\"\" loudly",
        "run of three \"\"\" inside",
        "braces {literal} here",
        "close brace alone } is safe",
        "backslash \\ alone",
        "trailing backslash \\",
        "mixed: He said \"hi\" at {plot} with \\ backslash",
        "escaped brace pair: \\{literal}",
        "double backslash then brace: \\\\{literal}",
        "",
        "line1\nline2\ttabbed",
    ];
    for text in cases {
        let (quote_count, restored) = round_trip(text).unwrap_or_else(|error| panic!("{text:?}: {error}"));
        assert_eq!(restored, text, "round trip changed text (quote_count={quote_count})");
        if text.contains('"') {
            assert!(quote_count >= 3, "interior quotes need a quote run of at least 3, got {quote_count} for {text:?}");
        }
    }
}

#[test]
fn boundary_quotes_and_control_characters_fail_closed() {
    for text in ["\"leading", "trailing\"", "\"both\"", "\"", "carriage\rreturn"] {
        let error = encode_v_text_literal(text).expect_err("unrepresentable title must fail closed");
        assert!(error.to_string().contains("chart title"), "unexpected error for {text:?}: {error}");
    }
}
