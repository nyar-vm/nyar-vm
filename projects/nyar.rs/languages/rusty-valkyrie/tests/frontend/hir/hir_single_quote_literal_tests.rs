//! 单引号捕获字面量（字符/字节/raw）的 HIR lowering 与类型归属测试。
//!
//! 合同：词法只按等量引号串捕获（oak 侧），语义判定在 HIR lowering：
//! - 无前缀 cooked：解码 `\n \t \r \0 \\ \' \" \u{...}`，一字符归 `char`，多字符归 `Utf8`。
//! - `r` 前缀：raw 不解码；`b` 前缀：字节字面量（`\xNN` 可用），必须恰好一个字节。
//! - 字符/字节字面量对形参的提升规则与整数字面量同源，比较/算术不要求调用方写 `as`。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    compiler::hir::lowering::errors::SingleQuoteLiteralFailure,
    types::hir::{HirExprKind, HirLiteral, HirStatementKind, types::ValkyrieType},
};

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
    match first_let_initializer_kind(source) {
        HirExprKind::Literal(HirLiteral::Byte(value)) => assert_eq!(value, expected, "源码 {source}"),
        other => panic!("源码 {source} 必须归字节字面量，实际 {other:?}"),
    }
}

fn assert_text(source: &str, expected: &str) {
    match first_let_initializer_kind(source) {
        HirExprKind::Literal(HirLiteral::String(literal)) => {
            assert_eq!(literal.segments.len(), 1, "源码 {source} 必须是单段文本");
            let rusty_valkyrie::types::hir::HirStringSegment::Text(text) = &literal.segments[0]
            else {
                panic!("源码 {source} 必须是文本段")
            };
            assert_eq!(text, expected, "源码 {source}");
        }
        other => panic!("源码 {source} 必须归字符串字面量，实际 {other:?}"),
    }
}

/// 反例：编译必须失败且是结构化 `InvalidSingleQuoteLiteral`，返回失败原因与 span。
fn expect_literal_failure(source: &str) -> (SingleQuoteLiteralFailure, usize, usize) {
    let error = ValkyrieCompiler::default().compile_source(source).expect_err(&format!("源码 {source} 必须失败"));
    match error {
        HirError::InvalidSingleQuoteLiteral { failure, span, .. } => (failure, span.start, span.end),
        other => panic!("源码 {source} 必须是 InvalidSingleQuoteLiteral，实际 {other:?}"),
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
    assert_character("micro probe() { let zero: char = '\\0' }", 0);
    assert_character("micro probe() { let slash: char = '\\\\' }", 92);
    assert_character("micro probe() { let quote: char = '\\\"' }", 34);
    assert_character("micro probe() { let alpha: char = '\\u{41}' }", 65);
}

#[test]
fn raw_single_quote_literal_skips_escapes() {
    // raw 不解析转义：`r'\'` 是单个反斜杠字符。
    assert_character("micro probe() { let slash: char = r'\\' }", 92);
    // raw 长内容归字符串，内容原样保留反斜杠。
    assert_text("micro probe() { let text = r'a\\nb' }", "a\\nb");
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
    assert!(matches!(failure, SingleQuoteLiteralFailure::EmptyByteLiteral));
}

#[test]
fn multi_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'ab' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::ByteLiteralNotSingleByte { count: 2 }));
}

#[test]
fn non_ascii_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'é' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::ByteLiteralNotAscii { .. }));
}

#[test]
fn unicode_escape_in_byte_literal_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = b'\\u{41}' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::UnicodeEscapeInByteLiteral));
}

#[test]
fn invalid_escape_fails_with_span() {
    let source = "micro probe() { let bad = '\\q' }";
    let (failure, start, end) = expect_literal_failure(source);
    assert!(matches!(failure, SingleQuoteLiteralFailure::InvalidEscapeSequence { .. }));
    // span 必须定位到 `\\q` 序列本身，而不是整个函数。
    assert_eq!(&source[start..end], "\\q");
}

#[test]
fn invalid_unicode_escape_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\u{}' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::InvalidUnicodeEscape { .. }));
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\u{110000}' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::InvalidUnicodeEscape { .. }));
}

#[test]
fn character_beyond_bmp_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '\\u{1F600}' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::CharacterBeyondBmp { value: 0x1F600 }));
    // 裸写超出 BMP 的字符同样拒绝，不静默截断。
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = '🙂' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::CharacterBeyondBmp { .. }));
}

#[test]
fn unknown_prefix_fails() {
    let (failure, _, _) = expect_literal_failure("micro probe() { let bad = x'0' }");
    assert!(matches!(failure, SingleQuoteLiteralFailure::UnknownPrefix { .. }));
}

#[test]
fn delimiter_escape_cannot_be_rescued() {
    // `'\''` 非法：捕获在第二个引号处闭合，残余引号未闭合；必须报未闭合，不得救转义。
    let error = ValkyrieCompiler::default().compile_source("micro probe() { let bad = '\\'' }").expect_err("转义定界符必须失败");
    let message = error.to_string();
    assert!(message.contains("unterminated quoted literal"), "必须报未闭合捕获：{message}");
    // `"\""` 同理。
    let error = ValkyrieCompiler::default().compile_source("micro probe() { let bad = \"\\\"\" }").expect_err("转义定界符必须失败");
    assert!(error.to_string().contains("unterminated quoted literal"));
}

#[test]
fn unterminated_capture_reports_distinct_diagnostic() {
    let error = ValkyrieCompiler::default().compile_source("micro probe() { let bad = '\\").expect_err("未闭合必须失败");
    assert!(error.to_string().contains("unterminated quoted literal: opening run of 1 `'`"));
}

// ---------- 类型规则：字面量提升与比较/算术互通 ----------

#[test]
fn byte_literal_specializes_to_numeric_formal() {
    // `b' '` 对 `u32` 形参直接绑定，不要求调用方写 `as u32`。
    let compiler = ValkyrieCompiler::default();
    let hir = compiler
        .compile_source(
            r#"
micro takes_u32(value: u32): bool { return true }
micro probe(): bool { return takes_u32(b' ') }
"#,
        )
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
        .compile_source(
            r#"
micro takes_char(value: char): bool { return true }
micro takes_u32(value: u32): bool { return true }
micro probe(): bool { return takes_char(' ') }
micro probe_numeric(): bool { return takes_u32('\t') }
"#,
        )
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
        .compile_source("micro probe() { let space: u32 = ' ' let tab: u8 = b'\\t' let mark: char = '#' }")
        .expect("let 注解必须直接绑定字符/字节字面量");
}
