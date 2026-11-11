//! 引号字面量的上下文定型与无约束默认（§4.10 B/D/E，2026-11-11 裁定）。
//!
//! 上下文先解约束、最后才默认：文本上下文接受单引号捕获（任意长度），
//! `char` 上下文接受恰好一个 BMP 字符的双引号捕获；确无上下文时单引号默认
//! `char`、双引号默认 `utf8`，`''`/`'ab'`/超 BMP 无约束即表示错误，不按长度回退。
//! 插值段不得渲染 `${...}` 占位，必须结构化失败点名缺 formatter 实例。

use rusty_valkyrie::{MirLowerer, ValkyrieCompiler, compiler::mir::ssa::MirOperation};

const PRIMITIVES: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::text::char")]
structure char { }

[primitive("std::text::utf8")]
structure utf8 { }
"#;

fn wrap(body: &str) -> String {
    format!("{PRIMITIVES}\n{body}")
}

/// 降低到 MIR 模块（不跑校验），用于检查常量承载形状。
fn lower(source: &str) -> rusty_valkyrie::compiler::mir::ssa::MirModule {
    let hir = ValkyrieCompiler::default().compile_source(&wrap(source)).unwrap_or_else(|error| panic!("{source}\n{error}"));
    MirLowerer::lower_module_semantic(&hir)
}

/// 走完整校验到 Semantic MIR；失败返回错误文本。
fn compile_checked(source: &str) -> Result<(), String> {
    ValkyrieCompiler::default().compile_source_to_mir(&wrap(source)).map(|_| ()).map_err(|error| error.to_string())
}

fn utf8_constants(module: &rusty_valkyrie::compiler::mir::ssa::MirModule, name: &str) -> Vec<String> {
    module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains(name))
        .expect("probe 函数")
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::LoadConstant { constant: rusty_valkyrie::compiler::mir::ssa::MirConstant::Utf8(text), .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn int_char_constants(module: &rusty_valkyrie::compiler::mir::ssa::MirModule, name: &str) -> Vec<i64> {
    module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains(name))
        .expect("probe 函数")
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::LoadConstant { constant: rusty_valkyrie::compiler::mir::ssa::MirConstant::Int(value), ty }
                if matches!(ty, Some(rusty_valkyrie::compiler::types::hir::types::ValkyrieType::Character)) =>
            {
                Some(*value)
            }
            _ => None,
        })
        .collect()
}

// ---------- 正例：文本上下文接受单引号捕获 ----------

#[test]
fn text_context_accepts_single_quote_char() {
    let module = lower("micro probe(): i32 { let t: utf8 = 'x' return 0 }");
    assert!(utf8_constants(&module, "probe").contains(&"x".to_string()), "单引号单字符在 utf8 上下文按文本落常量");
}

#[test]
fn text_context_accepts_single_quote_multi_and_empty() {
    let multi = lower("micro probe(): i32 { let t: utf8 = 'ab' return 0 }");
    assert!(utf8_constants(&multi, "probe").contains(&"ab".to_string()));
    let empty = lower("micro probe(): i32 { let t: utf8 = '' return 0 }");
    assert!(utf8_constants(&empty, "probe").contains(&String::new()), "空单引号在文本上下文是空文本");
}

// ---------- 正例：char 上下文接受恰好一个 BMP 字符的双引号捕获 ----------

#[test]
fn char_context_accepts_single_char_double_quote() {
    let module = lower("micro probe(): i32 { let c: char = \"x\" return 0 }");
    assert!(int_char_constants(&module, "probe").contains(&120), "`let c: char = \"x\"` 必须按码元落 char");
}

#[test]
fn char_context_accepts_single_quote_char() {
    let module = lower("micro probe(): i32 { let c: char = 'x' return 0 }");
    assert!(int_char_constants(&module, "probe").contains(&120));
}

// ---------- 正例：形参上下文双向绑定 ----------

#[test]
fn formal_contexts_bind_both_directions() {
    compile_checked(
        r#"
micro takes_text(value: utf8): bool { return true }
micro takes_char(value: char): bool { return true }
micro probe(): bool { return takes_text('x') && takes_text('ab') && takes_char("x") }
"#,
    )
    .expect("文本形参收单引号捕获、char 形参收单字符双引号捕获都必须成立");
}

// ---------- 反例：无约束单引号默认 char 后表示错误，不回退文本 ----------

#[test]
fn unconstrained_multi_char_single_quote_is_char_default_error() {
    let error = compile_checked("micro probe(): i32 { let t = 'ab' return 0 }").expect_err("无约束 `'ab'` 默认 char 必须表示错误");
    assert!(error.contains("char"), "必须点名 char 默认：{error}");
}

#[test]
fn unconstrained_empty_single_quote_is_char_default_error() {
    compile_checked("micro probe(): i32 { let t = '' return 0 }").expect_err("无约束 `''` 默认 char 必须表示错误");
}

#[test]
fn unconstrained_beyond_bmp_single_quote_is_char_default_error() {
    compile_checked("micro probe(): i32 { let t = '\u{1F642}' return 0 }").expect_err("无约束超 BMP 单引号默认 char 必须表示错误");
}

// ---------- 反例：char 上下文收到非单字符文本 ----------

#[test]
fn char_context_rejects_multi_char_capture() {
    compile_checked("micro probe(): i32 { let c: char = \"ab\" return 0 }").expect_err("char 上下文收到多字符文本必须失败");
}

// ---------- 反例：插值不得渲染 ${...} 占位，必须结构化失败 ----------

#[test]
fn interpolation_fails_closed_without_placeholder() {
    let error = compile_checked("micro probe(): i32 { let t = \"x{1}y\" return 0 }").expect_err("插值必须有 formatter 实例，否则失败");
    assert!(error.contains("formatter") || error.contains("display"), "必须点名缺 formatter/display 实例：{error}");
    assert!(!error.contains("${"), "绝不允许 dollar-brace 占位文本：{error}");
}

#[test]
fn interpolation_arithmetic_is_not_const_folded_to_placeholder() {
    // `"{ 3 + 1 }"` 需要真实 formatter 求值；本轮无实例，必须失败而不是产出文本或占位。
    let error = compile_checked("micro probe(): i32 { let t = \"{3 + 1}\" return 0 }").expect_err("插值算术无 formatter 实例必须失败");
    assert!(!error.contains("${") && !error.contains("4"), "不得伪装成常量文本或占位：{error}");
}
