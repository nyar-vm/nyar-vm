//! 引号捕获字面量（字符/字节/raw/模板）的 HIR lowering 与归属测试。
//!
//! 合同（§4.10，2026-11-11 用户裁定）：词法只按等量引号串捕获（oak 侧），
//! 语义判定在 HIR lowering，单双引号共用同一管线：
//! - 无前缀 cooked：标准转义集 `\" \\ \/ \b \f \n \r \t \uXXXX` 与 `\'`、`\{`、`\}`；
//!   未知/残缺转义与模板语法错误结构化报错。
//! - `r` 前缀：raw 不解码、不插值；`b` 前缀：字节字面量（`\xNN` 可用），必须恰好一个字节。
//! - 反斜杠不救定界符：`'\''`、`"\""` 非法；`"'"`、`'"'` 是引号字符的正确写法。
//! - 过渡归属：无插值时单引号恰好一字符归 `char`，其余归文本；完整上下文定型门另行落地。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    compiler::{
        diagnostics::pending,
        hir::lowering::errors::QuoteLiteralFailure,
    },
    types::hir::{HirExprKind, HirLiteral, HirStatementKind, types::ValkyrieType},
};

/// 数值/文本原始类型不是语言内建，必须绑定到源码声明身份。
const PRIMITIVES: &str = r#"
[primitive("std::primitive::u8")]
structure u8 { }

[primitive("std::primitive::u32")]
structure u32 { }

[primitive("std::text::char")]
structure char { }
"#;

/// 编译 `micro probe() { let ... }` 并返回首个 let 初始化表达式的 kind。
fn first_let_initializer_kind(source: &str) -> HirExprKind {
    let hir = ValkyrieCompiler::default().compile_source(source).expect("字面量必须完成 HIR lowering");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("缺少 probe 函数");
    let statement = function.body.statements.first().expect("缺少 let 语句");
    let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind
    else {
        panic!("首语句必须是带初始化的 let：{:?}", statement.kind)
    };
    initializer.kind.clone()
}

fn assert_character(source: &str, expected: u16) {
    match first_let_initializer_kind(source) {
        HirExprKind::Literal(HirLiteral::Character(value)) => assert_eq!(value, expected, "源码 {source}"),
        other => panic!("源码 {source} 必须归字符字面量，实际 {other:?}"),
    }
}

fn assert_byte(source: &str, expected: u8) {
    let source = format!("{PRIMITIVES}\n{source}");
    match first_let_initializer_kind(&source) {
        HirExprKind::Literal(HirLiteral::Byte(value)) => assert_eq!(value, expected, "源码 {source}"),
        other => panic!("源码 {source} 必须归字节字面量，实际 {other:?}"),
    }
}

fn plain_text_of(kind: &HirExprKind) -> Vec<rusty_valkyrie::types::hir::HirStringSegment> {
    match kind {
        HirExprKind::Literal(HirLiteral::String(literal)) => literal.segments.clone(),
        other => panic!("必须归字符串字面量，实际 {other:?}"),
    }
}

fn assert_text(source: &str, expected: &str) {
    let segments = plain_text_of(&first_let_initializer_kind(source));
    assert_eq!(segments.len(), 1, "源码 {source} 必须是单段文本");
    let rusty_valkyrie::types::hir::HirStringSegment::Text(text) = &segments[0] else {
        panic!("源码 {source} 必须是文本段")
    };
    assert_eq!(text, expected, "源码 {source}");
}

/// 反例：编译必须失败且是结构化 `InvalidQuoteLiteral`，返回失败原因与 span。
fn expect_literal_failure(source: &str) -> (QuoteLiteralFailure, usize, usize) {
    let error = ValkyrieCompiler::default().compile_source(source).expect_err(&format!("源码 {source} 必须失败"));
    match error {
        HirError::InvalidQuoteLiteral { failure, span, .. } => (failure, span.start, span.end),
        other => panic!("源码 {source} 必须是 InvalidQuoteLiteral，实际 {other:?}"),
    }
}

// ---------- 正例：字符字面量 ----------

#[test]
fn plain_character_literals_bind_char() {
    assert_character("micro probe() { let space: char = ' ' }", 32);
    assert_character("micro probe() { let mark: char = '#' }", 35);
    assert_character("micro probe() { let tick: char = '`' }", 96);
}

#[test]
fn escaped_character_literals_decode() {
    assert_character("micro probe() { let tab: char = '\\t' }", 9);
    assert_character("micro probe() { let newline: char = '\\n' }", 10);
    assert_character("micro probe() { let cr: char = '\\r' }", 13);
    assert_character("micro probe() { let backspace: char = '\\b' }", 8);
    assert_character("micro probe() { let feed: char = '\\f' }", 12);
    assert_character("micro probe() { let slash: char = '\\\\' }", 92);
    assert_character("micro probe() { let solidus: char = '\\/' }", 47);
    assert_character("micro probe() { let quote: char = '\\\"' }", 34);
    assert_character("micro probe() { let alpha: char = '\\u0041' }", 65);
}

#[test]
fn brace_escapes_decode_to_literal_braces() {
    // `\{`、`\}` 是字面花括号的唯一写法；解码后花括号不再开启插值。
    assert_text("micro probe() { let text = 'a\\{b\\}c' }", "a{b}c");
    assert_text("micro probe() { let text = \"x\\{y\\}\" }", "x{y}");
}

#[test]
fn cooked_capture_interpolates_real_expression() {
    // 插值在两种引号下同形；旧实现把 `1` 整段当名字，新管线必须给出真实表达式。
    let segments = plain_text_of(&first_let_initializer_kind("micro probe() { let t = 'x{1}y' }"));
    assert_eq!(segments.len(), 3, "期望 Text/Interpolation/Text 三段");
    assert!(matches!(&segments[2], rusty_valkyrie::types::hir::HirStringSegment::Text(text) if text == "y"));
    let rusty_valkyrie::types::hir::HirStringSegment::Interpolation { expr, .. } = &segments[1] else {
        panic!("中段必须是插值")
    };
    assert!(matches!(expr.kind, HirExprKind::Literal(HirLiteral::Integer64(1))), "插值必须是真实表达式树，不是名字路径：{:?}", expr.kind);
}

#[test]
fn raw_single_quote_literal_skips_escapes() {
    // raw 不解析转义：`r'\'` 是单个反斜杠字符。
    assert_character("micro probe() { let slash: char = r'\\' }", 92);
    // raw 长内容归字符串，内容原样保留反斜杠，也不解析插值。
    assert_text("micro probe() { let text = r'a\\nb' }", "a\\nb");
    assert_text("micro probe() { let text = r'x{1}y' }", "x{1}y");
}

#[test]
fn cross_quote_forms_are_legal() {
    // 要表达单引号用 `"'"`，要表达两个双引号用 `'""'`。
    assert_text("micro probe() { let quote = \"'\" }", "'");
    assert_text("micro probe() { let quotes = '\"\"' }", "\"\"");
}

#[test]
fn multi_character_capture_becomes_text() {
    assert_text("micro probe() { let text = 'ab' }", "ab");
    assert_text("micro probe() { let empty = '' }", "");
}

// ---------- 正例：字节字面量 ----------

#[test]
fn byte_literals_decode_to_u8() {
    assert_byte("micro probe() { let zero: u8 = b'0' }", 48);
    assert_byte("micro probe() { let space: u8 = b' ' }", 32);
    assert_byte("micro probe() { let tab: u8 = b'\\t' }", 9);
    assert_byte("micro probe() { let newline: u8 = b'\\n' }", 10);
    assert_byte("micro probe() { let max: u8 = b'\\xFF' }", 255);
    assert_byte("micro probe() { let slash: u8 = b'\\\\' }", 92);
}

// ---------- 反例：结构化失败且带 span ----------

#[test]
fn empty_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'' }");
    assert!(matches!(failure, QuoteLiteralFailure::EmptyByteLiteral));
}

#[test]
fn multi_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'ab' }");
    assert!(matches!(failure, QuoteLiteralFailure::ByteLiteralNotSingleByte { count: 2 }));
}

#[test]
fn non_ascii_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'é' }");
    assert!(matches!(failure, QuoteLiteralFailure::ByteLiteralNotAscii { .. }));
}

#[test]
fn unicode_escape_in_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'\\u0041' }");
    assert!(matches!(failure, QuoteLiteralFailure::UnicodeEscapeInByteLiteral));
}

#[test]
fn invalid_escape_fails_with_span() {
    let source = "micro probe() { let bad = '\\q' }";
    let (failure, start, end) = expect_literal_failure(source);
    assert!(matches!(failure, QuoteLiteralFailure::InvalidEscapeSequence { .. }));
    // span 必须定位到 `\\q` 序列本身，而不是整个函数。
    assert_eq!(&source[start..end], "\\q");
}

#[test]
fn zero_and_braced_unicode_escapes_are_not_in_the_set() {
    // `\0` 与 `\u{...}` 不在标准集内：按未知转义/模板语法拒绝，不静默保留。
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\0' }");
    assert!(matches!(failure, QuoteLiteralFailure::InvalidEscapeSequence { escape } if escape == "\\0"));
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\u{41}' }");
    assert!(matches!(failure, QuoteLiteralFailure::InvalidUnicodeEscape { .. }));
}

#[test]
fn invalid_unicode_escape_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\uZZZZ' }");
    assert!(matches!(failure, QuoteLiteralFailure::InvalidUnicodeEscape { .. }));
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\uD800' }");
    assert!(matches!(failure, QuoteLiteralFailure::InvalidUnicodeEscape { .. }));
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\u41' }");
    assert!(matches!(failure, QuoteLiteralFailure::InvalidUnicodeEscape { .. }));
}

#[test]
fn character_beyond_bmp_fails() {
    // 裸写超出 BMP 的字符拒绝，不静默截断。
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '🙂' }");
    assert!(matches!(failure, QuoteLiteralFailure::CharacterBeyondBmp { value: 0x1F642 }));
}

#[test]
fn unknown_prefix_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = x'0' }");
    assert!(matches!(failure, QuoteLiteralFailure::UnknownPrefix { .. }));
}

#[test]
fn bare_closing_brace_fails_closed() {
    // 字面花括号必须写 `\{`/`\}`；裸 `}` 是模板语法错误，不悄悄保留。
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = \"a}b\" }");
    assert!(matches!(failure, QuoteLiteralFailure::TemplateSyntax { .. }));
}

#[test]
fn delimiter_escape_cannot_be_rescued() {
    // `'\''` 非法：捕获在第二个引号处闭合，残余引号未闭合；必须报未闭合，不得救转义。
    // 根循环恢复后失败不再致命：坏 item 被丢弃，未闭合原因记入诊断汇。
    pending::clear_pending_diagnostics();
    ValkyrieCompiler::default().compile_source("micro probe() { let bad = '\\'' }").expect("恢复后编译在恢复模块上继续");
    let collected = pending::take_pending_diagnostics();
    assert!(
        collected.diagnostics.iter().any(|diagnostic| diagnostic.message.contains("unterminated quoted literal")),
        "必须报未闭合捕获：{collected:?}"
    );
    // `"\""` 同理。
    pending::clear_pending_diagnostics();
    ValkyrieCompiler::default().compile_source("micro probe() { let bad = \"\\\"\" }").expect("恢复后编译在恢复模块上继续");
    let collected = pending::take_pending_diagnostics();
    assert!(
        collected.diagnostics.iter().any(|diagnostic| diagnostic.message.contains("unterminated quoted literal")),
        "必须报未闭合捕获：{collected:?}"
    );
}

#[test]
fn unterminated_capture_reports_distinct_diagnostic() {
    pending::clear_pending_diagnostics();
    ValkyrieCompiler::default().compile_source("micro probe() { let bad = '\\").expect("恢复后编译在恢复模块上继续");
    let collected = pending::take_pending_diagnostics();
    assert!(
        collected.diagnostics.iter().any(|diagnostic| diagnostic.message.contains("unterminated quoted literal: opening run of 1 `'`")),
        "必须带开引号数量的未闭合诊断：{collected:?}"
    );
}

// ---------- 类型规则：字面量提升与比较/算术互通 ----------

#[test]
fn byte_literal_specializes_to_numeric_formal() {
    // `b' '` 对 `u32` 形参直接绑定，不要求调用方写 `as u32`。
    let compiler = ValkyrieCompiler::default();
    let hir = compiler
        .compile_source(&format!(
            "{PRIMITIVES}\n{}",
            r#"
micro takes_u32(value: u32): bool { return true }
micro probe(): bool { return takes_u32(b' ') }
"#
        ))
        .expect("字节字面量必须按数值形参特化");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").unwrap();
    let HirExprKind::Return(Some(result)) = &function.body.expr.as_ref().unwrap().kind
    else {
        panic!("probe 必须 return 调用")
    };
    let HirExprKind::Call { resolved: Some(call), .. } = &result.kind
    else {
        panic!("return 必须是已解析调用")
    };
    // 解析成功即证明特化发生（未特化的 `u8` 实参配不上 `u32` 形参）；
    // 冻结的形参类型保持声明写法 `u32`。
    let formal = call.parameter_types.first().expect("takes_u32 必须有一个形参");
    assert!(
        matches!(formal, ValkyrieType::Named(name) if name.as_str() == "u32") || matches!(formal, ValkyrieType::Integer32 { signed: false }),
        "形参必须是 u32，实际 {formal:?}"
    );
}

#[test]
fn character_literal_binds_char_and_numeric_formals() {
    let compiler = ValkyrieCompiler::default();
    let hir = compiler
        .compile_source(&format!(
            "{PRIMITIVES}\n{}",
            r#"
micro takes_char(value: char): bool { return true }
micro takes_u32(value: u32): bool { return true }
micro probe(): bool { return takes_char(' ') }
micro probe_numeric(): bool { return takes_u32('\t') }
"#
        ))
        .expect("字符字面量必须同时绑定 char 与数值形参");
    let probe = hir.functions.iter().find(|function| function.name.as_str() == "probe").unwrap();
    let HirExprKind::Return(Some(result)) = &probe.body.expr.as_ref().unwrap().kind
    else {
        panic!("probe 必须 return 调用")
    };
    let HirExprKind::Call { resolved: Some(call), .. } = &result.kind
    else {
        panic!("return 必须是已解析调用")
    };
    let formal = call.parameter_types.first().expect("takes_char 必须有一个形参");
    assert!(
        matches!(formal, ValkyrieType::Character) || matches!(formal, ValkyrieType::Named(name) if name.as_str() == "char"),
        "形参必须是 char，实际 {formal:?}"
    );

    let probe_numeric = hir.functions.iter().find(|function| function.name.as_str() == "probe_numeric").unwrap();
    let HirExprKind::Return(Some(result)) = &probe_numeric.body.expr.as_ref().unwrap().kind
    else {
        panic!("probe_numeric 必须 return 调用")
    };
    let HirExprKind::Call { resolved: Some(call), .. } = &result.kind
    else {
        panic!("return 必须是已解析调用")
    };
    let formal = call.parameter_types.first().expect("takes_u32 必须有一个形参");
    assert!(
        matches!(formal, ValkyrieType::Named(name) if name.as_str() == "u32") || matches!(formal, ValkyrieType::Integer32 { signed: false }),
        "形参必须是 u32，实际 {formal:?}"
    );
}

#[test]
fn byte_literal_does_not_bind_char_formal() {
    // 字节字面量不是字符：`char` 形参不得接受 `b'0'`。
    let result = ValkyrieCompiler::default().compile_source(
        r#"
micro takes_char(value: char): bool { return true }
micro probe(): bool { return takes_char(b'0') }
"#,
    );
    assert!(result.is_err(), "字节字面量不得绑定 `char` 形参");
}

#[test]
fn let_annotation_binds_literal_without_cast() {
    // `let` 注解位置同样走标量字面量绑定：`u32`/`u8`/`char` 都不要求 `as`。
    ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}\n{}", "micro probe() { let space: u32 = ' ' let tab: u8 = b'\\t' let mark: char = '#' }"))
        .expect("let 注解必须直接绑定字符/字节字面量");
}
