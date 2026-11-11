//! 引号字面量的上下文定型与无约束默认（§4.10 B/D/E，2026-11-11 裁定）。
//!
//! 上下文先解约束、最后才默认：文本上下文接受单引号捕获（任意长度），
//! `char` 上下文接受恰好一个码位的双引号捕获；确无上下文时单引号默认
//! `char`、双引号默认 `utf8`，`''`/`'ab'` 无约束即表示错误，不按长度回退；
//! `char` 为 u32 码位（§4.12 字符合同），非 BMP 单引号捕获是合法 `char`。
//! 插值在 HIR 脱糖为普通 `display` / `infix ++` 调用，MIR 只编码调用与常量。

use rusty_valkyrie::{MirLowerer, ValkyrieCompiler, compiler::mir::ssa::MirOperation};

const PRIMITIVES: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::text::char")]
structure char { }

[primitive("std::text::utf8")]
class Utf8Text { }

[primitive("std::text::utf16")]
class Utf16Text { }

type utf8 = Utf8Text;
type utf16 = Utf16Text;

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

imply i32 {
    infix `+`(self, rhs: Self): Self { return self }
}

imply Utf8Text {
    infix `++`(self, other: utf8): utf8 { return self }
}
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

fn utf16_constants(module: &rusty_valkyrie::compiler::mir::ssa::MirModule, name: &str) -> Vec<String> {
    module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains(name))
        .expect("probe 函数")
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::LoadConstant { constant: rusty_valkyrie::compiler::mir::ssa::MirConstant::Utf16(text), .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// probe 函数体内的调用指令数；插值必须落成真实 Invoke，而不是常量占位。
fn call_count(module: &rusty_valkyrie::compiler::mir::ssa::MirModule, name: &str) -> usize {
    module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains(name))
        .expect("probe 函数")
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter(|instruction| matches!(instruction.kind, MirOperation::Call { .. }))
        .count()
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

// ---------- 正例：char 上下文接受恰好一个码位的双引号捕获 ----------

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
fn unconstrained_beyond_bmp_single_quote_binds_char() {
    // §4.12 字符合同：`char` 为 u32 码位，非 BMP 单引号捕获按 char 默认合法定型，
    // 不再是表示错误（旧 BMP-only 拒绝合同已退役）。
    let module = lower("micro probe(): i32 { let t = '\u{1F642}' return 0 }");
    assert!(int_char_constants(&module, "probe").contains(&128578), "非 BMP 单引号必须按 u32 码位落 char 常量");
}

// ---------- 反例：char 上下文收到非单字符文本 ----------

#[test]
fn char_context_rejects_multi_char_capture() {
    compile_checked("micro probe(): i32 { let c: char = \"ab\" return 0 }").expect_err("char 上下文收到多字符文本必须失败");
}

// ---------- 正例：插值脱糖为普通调用，不产出占位文本 ----------

#[test]
fn interpolation_lowers_to_display_call_not_placeholder() {
    // `"{ 1 }"` 在 MIR 必须是已绑定 `display` 的普通 Invoke，绝不允许 `${...}` 占位常量。
    let module = lower("micro probe(): utf8 { let t = \"{ 1 }\" return t }");
    assert!(call_count(&module, "probe") >= 1, "插值必须落成普通调用");
    assert!(utf8_constants(&module, "probe").iter().all(|text| !text.contains('{') && !text.contains("${")), "不得渲染插值源文或占位文本");
}

#[test]
fn interpolation_multi_segment_keeps_left_to_right_calls() {
    // `"a{ x }b"`：`display` + 两次 `infix ++`，文本段仍按源码顺序落常量。
    let module = lower("micro probe(x: i32): utf8 { let t = \"a{ x }b\" return t }");
    assert!(call_count(&module, "probe") >= 3, "多段模板必须保留 display 与两次字符串连接调用");
    let texts = utf8_constants(&module, "probe");
    assert!(texts.contains(&"a".to_string()) && texts.contains(&"b".to_string()), "文本段顺序与内容必须保留：{texts:?}");
}

#[test]
fn interpolation_arithmetic_is_real_call_not_constant_text() {
    // `"{ 3 + 1 }"` 的 `infix +` 是真实调用；MIR 不得出现折叠成 `"4"` 的文本常量。
    let module = lower("micro probe(): utf8 { let t = \"{ 3 + 1 }\" return t }");
    assert!(call_count(&module, "probe") >= 2, "算术表达式与 display 都必须是普通调用");
    assert!(!utf8_constants(&module, "probe").contains(&"4".to_string()), "不得以文本常量冒充求值");
}

#[test]
fn interpolation_without_display_fails_with_display_diagnostic() {
    // 缺少 Display 实例时必须由既有调用解析结构化失败，错误点名 `display`。
    let error = compile_checked("micro probe(w: Missing): utf8 { return \"{ w }\" }").expect_err("缺少 Display 实例必须失败");
    assert!(error.contains("display"), "必须点名 display：{error}");
    assert!(!error.contains("${"), "不得渲染 dollar-brace 占位：{error}");
}

// ---------- 正例：文本编码按已冻结载体落常量 ----------

#[test]
fn settled_utf16_text_encoding_lowers_to_utf16_constant() {
    // HIR settle 冻结的 `text_encoding` 是 MIR 唯一依据：utf16 上下文必须落 Utf16 常量。
    let module = lower("micro probe(): i32 { let t: utf16 = \"x\" return 0 }");
    assert!(utf16_constants(&module, "probe").contains(&"x".to_string()), "utf16 文本必须落 Utf16 常量");
}
