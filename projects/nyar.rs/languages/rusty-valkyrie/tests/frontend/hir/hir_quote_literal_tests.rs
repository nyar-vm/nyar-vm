//! 引号捕获字面量（字符/字节/raw/模板）的 HIR lowering 与归属测试。
//!
//! 合同（§4.10，2026-11-11 用户裁定）：词法只按等量引号串捕获（oak 侧），
//! 语义判定在 HIR lowering，单双引号共用同一管线：
//! - 无前缀 cooked：标准转义集 `\" \\ \/ \b \f \n \r \t \uXXXX` 与 `\'`、`\{`、`\}`；
//!   未知/残缺转义与模板语法错误结构化报错。
//! - `r` 前缀：raw 不解码、不插值；`b` 前缀：字节字面量（`\xNN` 可用），必须恰好一个字节。
//! - 反斜杠不救定界符：`'\''`、`"\""` 非法；`"'"`、`'"'` 是引号字符的正确写法。
//! - 归属（§4.10 B / §4.12 字符合同）：捕获解码后一律保持未定型形态，由 HIR 结算
//!   pass 按上下文唯一型定：char 目标落 `Character`（u32 码位，非 BMP 合法），
//!   文本目标冻结编码载体；无约束 `''`/`'ab'` 按 `char` 默认后报表示错误，不提供
//!   长度回退。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    compiler::{diagnostics::pending, hir::lowering::errors::QuoteLiteralFailure},
    types::hir::{
        HirCallResolutionFailureKind, HirExpr, HirExprKind, HirLiteral, HirModule, HirResolvedCall, HirStatementKind, HirTextEncoding,
        types::ValkyrieType,
    },
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

fn assert_character(source: &str, expected: u32) {
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
    let rusty_valkyrie::types::hir::HirStringSegment::Text(text) = &segments[0]
    else {
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
    // 多码位单引号捕获无约束即表示错误，文本位必须显式注解（§4.10 B）。
    assert_text("micro probe() { let text: utf8 = 'a\\{b\\}c' }", "a{b}c");
    assert_text("micro probe() { let text = \"x\\{y\\}\" }", "x{y}");
}

/// 插值脱糖只用既有正式声明：Display trait、文本载体与 `infix +` 都是普通 V 声明，
/// 编译器不按类型名白名单识别 formatter。
const INTERPOLATION_FIXTURE: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::text::char")]
structure char { }

[primitive("std::text::utf8")]
class Utf8Text { }

type utf8 = Utf8Text;

class Widget { value: i32 }

class Missing { }

trait Display {
    micro display(self): utf8
}

imply i32: Display {
    micro display(self): utf8 { return "0" }
}

imply Utf8Text: Display {
    micro display(self): utf8 { return self }
}

imply Widget: Display {
    micro display(self): utf8 { return "widget" }
}

imply i32 {
    infix `+`(self, rhs: Self): Self { return self }
}

imply Utf8Text {
    infix `++`(self, other: utf8): utf8 { return self }
}
"#;

fn compile_with_interpolation_fixture(source: &str) -> Result<HirModule, HirError> {
    ValkyrieCompiler::default().compile_source(&format!("{INTERPOLATION_FIXTURE}\n{source}"))
}

fn function_return_expression<'a>(hir: &'a HirModule, name: &str) -> &'a HirExpr {
    let function = hir.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 {name} 函数"));
    let HirExprKind::Return(Some(value)) = &function.body.expr.as_ref().unwrap_or_else(|| panic!("{name} 缺少尾表达式")).kind
    else {
        panic!("{name} 必须以 return 收尾")
    };
    value.as_ref()
}

fn resolved_call(expr: &HirExpr) -> &HirResolvedCall {
    let HirExprKind::Call { resolved: Some(call), .. } = &expr.kind
    else {
        panic!("表达式必须是已解析调用，实际 {:?}", expr.kind)
    };
    call
}

fn text_segment(expr: &HirExpr) -> &str {
    let HirExprKind::Literal(HirLiteral::String(literal)) = &expr.kind
    else {
        panic!("段必须是文本字面量，实际 {:?}", expr.kind)
    };
    assert_eq!(literal.text_encoding, Some(HirTextEncoding::Utf8), "文本段必须冻结 utf8 编码");
    let [rusty_valkyrie::types::hir::HirStringSegment::Text(text)] = literal.segments.as_slice()
    else {
        panic!("文本段必须是单一文本片段，实际 {:?}", literal.segments)
    };
    text
}

fn call_arguments(expr: &HirExpr) -> &[rusty_valkyrie::types::hir::HirCallArgument] {
    let HirExprKind::Call { args, .. } = &expr.kind
    else {
        panic!("表达式必须是调用，实际 {:?}", expr.kind)
    };
    args
}

#[test]
fn interpolation_single_slot_binds_display_call() {
    // `"{ 1 }"` 没有文本段，直接返回 `1.display()`；不再是字符串字面量或占位文本。
    let hir = compile_with_interpolation_fixture(r#"micro probe(): utf8 { return "{ 1 }" }"#).expect("单插值必须完成 HIR 解析");
    let expr = function_return_expression(&hir, "probe");
    let call = resolved_call(expr);
    assert_eq!(
        call.symbol.parts().last().map(|identifier| identifier.as_str()),
        Some("display"),
        "必须绑定声明身份 `display`，实际 {}",
        call.symbol
    );
    assert!(call_arguments(expr).is_empty(), "display 的显式实参为空，接收者由成员调用承载");
    let HirExprKind::Call { callee, .. } = &expr.kind
    else {
        unreachable!()
    };
    let HirExprKind::FieldAccess { object, field, .. } = &callee.kind
    else {
        panic!("display 必须以成员选择为 callee，实际 {:?}", callee.kind)
    };
    assert_eq!(field.as_str(), "display");
    assert!(matches!(object.kind, HirExprKind::Literal(HirLiteral::Integer(1))), "插值表达式必须是真实表达式树");
}

#[test]
fn interpolation_segments_concatenate_left_to_right() {
    // `"a{ x }b"` 改写为 `("a" ++ x.display()) ++ "b"`，段顺序与源码一致。
    let hir = compile_with_interpolation_fixture(r#"micro probe(x: i32): utf8 { return "a{ x }b" }"#).expect("多段模板必须完成 HIR 解析");
    let expr = function_return_expression(&hir, "probe");
    let outer = resolved_call(expr);
    assert_eq!(outer.symbol.parts().last().map(|identifier| identifier.as_str()), Some("infix ++"));
    let outer_args = call_arguments(expr);
    assert_eq!(outer_args.len(), 2);
    assert_eq!(text_segment(&outer_args[1].value), "b", "右端文本段必须是源码后段");
    let inner = &outer_args[0].value;
    assert_eq!(resolved_call(inner).symbol.parts().last().map(|identifier| identifier.as_str()), Some("infix ++"));
    let inner_args = call_arguments(inner);
    assert_eq!(text_segment(&inner_args[0].value), "a");
    let display = &inner_args[1].value;
    let display_call = resolved_call(display);
    assert_eq!(display_call.symbol.parts().last().map(|identifier| identifier.as_str()), Some("display"));
    let HirExprKind::Call { callee, .. } = &display.kind
    else {
        unreachable!()
    };
    let HirExprKind::FieldAccess { object, .. } = &callee.kind
    else {
        unreachable!()
    };
    assert!(matches!(&object.kind, HirExprKind::Variable(identifier) if identifier.name.as_str() == "x"), "插值段必须保留原变量");
}

#[test]
fn interpolation_arithmetic_uses_declared_operator() {
    // `"{ 3 + 1 }"` 的插值表达式是已解析的 `i32.infix +` 调用，不折叠成文本。
    let hir = compile_with_interpolation_fixture(r#"micro probe(): utf8 { return "{ 3 + 1 }" }"#).expect("算术插值必须完成 HIR 解析");
    let expr = function_return_expression(&hir, "probe");
    let display_call = resolved_call(expr);
    assert_eq!(display_call.symbol.parts().last().map(|identifier| identifier.as_str()), Some("display"));
    let HirExprKind::Call { callee, .. } = &expr.kind
    else {
        unreachable!()
    };
    let HirExprKind::FieldAccess { object, .. } = &callee.kind
    else {
        unreachable!()
    };
    let addition = resolved_call(object);
    assert_eq!(addition.symbol.parts().last().map(|identifier| identifier.as_str()), Some("infix +"), "插值必须先解出声明运算符");
    let operands = call_arguments(object);
    assert_eq!(operands.len(), 2);
    assert!(matches!(operands[0].value.kind, HirExprKind::Literal(HirLiteral::Integer(3))));
    assert!(matches!(operands[1].value.kind, HirExprKind::Literal(HirLiteral::Integer(1))));
}

#[test]
fn interpolation_binds_user_defined_display() {
    // 用户声明身份上的 Display 实现与内建类型走同一解析路径。
    let hir = compile_with_interpolation_fixture(r#"micro probe(w: Widget): utf8 { return "{ w }" }"#).expect("自定义 Display 必须可解析");
    let call = resolved_call(function_return_expression(&hir, "probe"));
    assert_eq!(call.symbol.parts().last().map(|identifier| identifier.as_str()), Some("display"));
    assert!(call.instance.is_some(), "具体 impl 成员必须冻结实例身份");
}

#[test]
fn interpolation_without_display_fails_closed() {
    // 没有 Display 实例时由既有调用解析结构化失败，错误点名 `display`，不得产出占位文本。
    let error =
        compile_with_interpolation_fixture(r#"micro probe(w: Missing): utf8 { return "{ w }" }"#).expect_err("缺少 Display 实例必须失败");
    let rendered = error.to_string();
    assert!(rendered.contains("display"), "失败必须点名 display：{rendered}");
    assert!(!rendered.contains("${"), "不得渲染 dollar-brace 占位：{rendered}");
    match error {
        HirError::UnresolvedCall { span, .. } => assert!(span.start < span.end, "失败必须带源区间"),
        other => panic!("必须是结构化未解析调用，实际 {other:?}"),
    }
}

#[test]
fn raw_single_quote_literal_skips_escapes() {
    // raw 不解析转义：`r'\'` 是单个反斜杠字符。
    assert_character("micro probe() { let slash: char = r'\\' }", 92);
    // raw 长内容归字符串（文本位显式注解），内容原样保留反斜杠，也不解析插值。
    assert_text("micro probe() { let text: utf8 = r'a\\nb' }", "a\\nb");
    assert_text("micro probe() { let text: utf8 = r'x{1}y' }", "x{1}y");
}

#[test]
fn cross_quote_forms_are_legal() {
    // 要表达单引号用 `"'"`，要表达两个双引号用 `'""'`（文本位显式注解）。
    assert_text("micro probe() { let quote = \"'\" }", "'");
    assert_text("micro probe() { let quotes: utf8 = '\"\"' }", "\"\"");
}

#[test]
fn unconstrained_multi_and_empty_single_quote_fail_at_hir_settle() {
    // §4.10 B：无约束 `''`/`'ab'` 按 char 默认后报表示错误；定型是 HIR 唯一门，
    // 不再放行到 MIR，也不按长度回退文本。
    for source in ["micro probe() { let text = 'ab' }", "micro probe() { let empty = '' }"] {
        let error = ValkyrieCompiler::default().compile_source(source).expect_err(&format!("源码 {source} 必须在 HIR 结算失败"));
        match error {
            HirError::InvalidQuoteLiteral { failure, .. } => {
                assert!(matches!(failure, QuoteLiteralFailure::CharDefaultNotRepresentable { .. }), "源码 {source} 实际 {failure:?}");
            }
            other => panic!("源码 {source} 必须是 InvalidQuoteLiteral，实际 {other:?}"),
        }
    }
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
fn invalid_hex_escape_with_non_ascii_suffix_does_not_panic() {
    // 与 `\u` 同一 panic 边界：`\x1é` 的两位十六进制只收到 1 位便遇非 ASCII；旧实现按
    // 任意字节 `hex_end` 切片会落在 `é` 的 UTF-8 字符内部 panic。现在结构化报
    // `InvalidEscapeSequence`，不得越界切片。
    let (failure, _, _) = expect_literal_failure(r#"micro probe() { let bad = b'\x1é' }"#);
    assert!(matches!(failure, QuoteLiteralFailure::InvalidEscapeSequence { .. }));
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
fn invalid_unicode_escape_with_non_ascii_suffix_does_not_panic() {
    // S5 panic 边界：`\u123é` 的十六进制位只收集到 3 位便遇非 ASCII；旧实现按任意字节
    // `end` 切片，`end` 落在 `é` 的 UTF-8 字符内部会 panic。现在必须结构化报
    // `InvalidUnicodeEscape`，不得越界切片。
    let (failure, _, _) = expect_literal_failure(r#"micro probe() { let bad = "\u123éx" }"#);
    assert!(matches!(failure, QuoteLiteralFailure::InvalidUnicodeEscape { .. }));
    let (failure, _, _) = expect_literal_failure(r#"micro probe() { let bad = "\u12é" }"#);
    assert!(matches!(failure, QuoteLiteralFailure::InvalidUnicodeEscape { .. }));
}

#[test]
fn valid_unicode_escape_followed_by_non_ascii_decodes() {
    // 合法 `\uXXXX` 后紧跟非 ASCII 文本仍正常解码：十六进制位不被后缀多字节字符污染。
    assert_text(r#"micro probe() { let text = "\u00e9é" }"#, "éé");
}

#[test]
fn character_beyond_bmp_binds_char() {
    // §4.12 字符合同：`char` 为 u32 码位，非 BMP 单引号捕获是合法 `char` 字面量，
    // 按 `char` 默认落 Character，不再保持未定型文本捕获（旧 BMP-only 合同已退役）。
    assert_character("micro probe() { let emoji = '\u{1F642}' }", 0x1F642);
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
fn delimiter_escape_is_supported() {
    // cooked 单引号串里 `\` 保护定界符：`'\''` 是单引号字符，`"\""` 是单引号文本。
    // raw 前缀（`r'…'`）不参与该保护，仍按逐字闭合（见 `raw_single_quote_literal_skips_escapes`）。
    pending::clear_pending_diagnostics();
    assert_character("micro probe() { let quote = '\\'' }", 0x27);
    assert_text("micro probe() { let text = \"\\\"\" }", "\"");
    let collected = pending::take_pending_diagnostics();
    assert!(collected.diagnostics.is_empty(), "定界符转义不得产生恢复诊断：{collected:?}");
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

// ---------- 重载裁决：多合法候选报歧义，不按引号/长度打分 ----------

/// `char` 与文本形参并存的 `append` 组；两种引号在 §4.10 B 下都能落到文本形参，
/// 单码位捕获也能落到 `char` 形参，因此多个合法候选必须报歧义，禁止按引号/长度
/// 为候选打分（旧「绑定代价」旁路已清退）。
const OVERLOAD_APPEND_FIXTURE: &str = r#"
[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::text::char")]
structure char { }

[primitive("std::text::utf8")]
structure Utf8Text { }

structure Builder {
    flag: bool
}

imply Builder {
    micro append(self, c: char): bool {
        return true
    }

    micro append(self, text: Utf8Text): bool {
        return false
    }
}
"#;

/// 断言 `builder.append(<argument>)` 在 char/文本双合法时报歧义，不得静默选一个。
fn assert_append_ambiguous(argument: &str) {
    let source = format!(
        "{OVERLOAD_APPEND_FIXTURE}\nmicro probe(): bool {{ let builder: Builder = Builder {{ flag: true }} return builder.append({argument}) }}"
    );
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err(&format!("实参 {argument} 对 char/文本双形参必须报歧义"));
    // 断言结构化歧义语义，不匹配消息字符串：char/文本两个同具体度候选并列，
    // 解析器冻结 `AmbiguousExplicitImpls`，不得静默选一个。
    match error {
        HirError::UnresolvedCall { reason, .. } => {
            assert!(
                matches!(reason, HirCallResolutionFailureKind::AmbiguousExplicitImpls { candidates: 2 }),
                "实参 {argument} 必须冻结为双候选歧义，实际 {reason:?}"
            );
        }
        other => panic!("实参 {argument} 必须是未解析调用歧义，实际 {other:?}"),
    }
}

/// 返回 `probe` 尾表达式 `return builder.append(...)` 的冻结形参表（含 receiver）。
fn resolved_append_formal_types(argument: &str) -> Vec<ValkyrieType> {
    let source = format!(
        "{OVERLOAD_APPEND_FIXTURE}\nmicro probe(): bool {{ let builder: Builder = Builder {{ flag: true }} return builder.append({argument}) }}"
    );
    let hir = ValkyrieCompiler::default().compile_source(&source).expect("字面量实参必须完成重载解析");
    let probe = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("缺少 probe 函数");
    let HirExprKind::Return(Some(result)) = &probe.body.expr.as_ref().expect("probe 缺少尾表达式").kind
    else {
        panic!("probe 必须 return 调用")
    };
    let HirExprKind::Call { resolved: Some(call), .. } = &result.kind
    else {
        panic!("return 必须是已解析调用，实际 {:?}", result.kind)
    };
    call.parameter_types.clone()
}

#[test]
fn single_code_point_captures_are_ambiguous_over_char_and_text() {
    // `'a'` 与 `"a"` 对 char/文本两个形参都是合法候选：报歧义，不按引号或长度打分。
    assert_append_ambiguous("'a'");
    assert_append_ambiguous("\"a\"");
    assert_append_ambiguous("'\\n'");
}

/// 真实 std 的文本模板形态：源码没有 `type utf8 = Utf8Text` 别名，formatter 与文本连接
/// 都由 `imply` 提供。调用解析必须按声明身份把内建 `Utf8` 载体与 `Utf8Text` 候选归一，
/// 不得依赖别名拼写或短名表。
const BUILTIN_TEXT_INTERPOLATION_FIXTURE: &str = r#"
[primitive("std::primitive::u8")]
structure u8 { }

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::text::utf8")]
class Utf8Text { _repr: [u8] }

trait Display {
    micro display(self): utf8
}

imply i64: Display {
    micro display(self): utf8 { return "0" }
}

imply Utf8Text: Display {
    micro display(self): utf8 { return self }
}

imply Utf8Text {
    infix `++`(self, other: utf8): utf8 { return self }
}
"#;

fn compile_builtin_text_interpolation(source: &str) -> HirModule {
    ValkyrieCompiler::default()
        .compile_source(&format!("{BUILTIN_TEXT_INTERPOLATION_FIXTURE}\n{source}"))
        .expect("内建文本载体上的插值必须完成 HIR 解析")
}

#[test]
fn interpolation_resolves_formatter_without_source_text_alias() {
    // `name`/`suffix` 的 `utf8` 是内建载体：`display` 必须命中 `imply Utf8Text: Display`，
    // 段间连接必须命中 `imply Utf8Text: infix ++`，两者都不靠源码别名拼写。
    let hir = compile_builtin_text_interpolation(r#"micro probe(name: utf8, suffix: utf8): utf8 { return "{ name }{ suffix }" }"#);
    let expr = function_return_expression(&hir, "probe");
    let concat = resolved_call(expr);
    assert_eq!(concat.symbol.parts().last().map(|identifier| identifier.as_str()), Some("infix ++"));
    let operands = call_arguments(expr);
    assert_eq!(operands.len(), 2, "两个插值段必须由一次 `infix ++` 连接");
    for operand in operands {
        let display = resolved_call(&operand.value);
        assert_eq!(display.symbol.parts().last().map(|identifier| identifier.as_str()), Some("display"));
        assert_eq!(display.return_type, ValkyrieType::Utf8, "formatter 返回类型必须冻结为文本载体");
        assert!(display.instance.is_some(), "具体 impl 成员必须冻结实例身份");
    }
}

#[test]
fn interpolation_call_result_concatenates_with_text_segment() {
    // 插值段是调用结果时，`bump()` 先作为普通调用结束求值，再进入 `display()` 与文本连接；
    // 连接候选必须按已冻结返回类型 `i64`/`utf8` 命中，而不是要求插值段是变量。
    let hir = compile_builtin_text_interpolation(
        r#"micro bump(counter: i64): i64 { return counter }
micro probe(): utf8 { return "x{ bump(1) }" }"#,
    );
    let expr = function_return_expression(&hir, "probe");
    let concat = resolved_call(expr);
    assert_eq!(concat.symbol.parts().last().map(|identifier| identifier.as_str()), Some("infix ++"));
    let operands = call_arguments(expr);
    assert_eq!(operands.len(), 2, "文本段与插值段必须由 `infix ++` 连接");
    let display = resolved_call(&operands[1].value);
    assert_eq!(display.symbol.parts().last().map(|identifier| identifier.as_str()), Some("display"));
    assert_eq!(display.return_type, ValkyrieType::Utf8);
    let HirExprKind::Call { callee, .. } = &operands[1].value.kind
    else {
        panic!("插值段必须是 display 调用")
    };
    let HirExprKind::FieldAccess { object, .. } = &callee.kind
    else {
        panic!("display 必须以成员选择为 callee")
    };
    let bump = resolved_call(object);
    assert_eq!(bump.symbol.parts().last().map(|identifier| identifier.as_str()), Some("bump"));
    assert_eq!(bump.return_type, ValkyrieType::Integer64 { signed: true }, "`bump` 的返回类型必须先冻结");
}

#[test]
fn multi_character_text_literal_prefers_text_formal() {
    // 多码位捕获只配得上文本形参：唯一合法候选正常解析，且冻结形参不是 char。
    let formal = resolved_append_formal_types("\"ab\"").get(1).cloned().expect("append 必须有文本形参");
    let is_char_formal = matches!(&formal, ValkyrieType::Character) || matches!(&formal, ValkyrieType::Named(name) if name.as_str() == "char");
    assert!(!is_char_formal, "多字符文本捕获必须选文本形参，实际 {formal:?}");
}
