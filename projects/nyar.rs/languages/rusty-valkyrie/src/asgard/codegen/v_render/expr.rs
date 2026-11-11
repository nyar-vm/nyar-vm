use crate::asgard::awsl::{ScriptBinding, SignalValueType, expr_util::split_awsl_ternary};

use super::{
    loop_frame::LoopFrame,
    util::{encode_v_text_literal, replace_ident, strip_expr_braces},
};

pub(crate) fn rewrite_utf8_expr_body(expr: &str, bindings: &[ScriptBinding], loops: &[LoopFrame]) -> String {
    rewrite_utf8_expr_body_guarded(expr, bindings, loops, &mut Vec::new())
}

/// 文本位置的类型投影：一律落 `Display` 合同的 `display()` 方法调用。
///
/// 历史上这里发 `utf8(<expr>)`，但 `utf8` 从来不是可调用声明（探针实测
/// `unresolved call utf8`）。std 已落 `Display`（整数 / `bool` / `char` / `utf8`）
/// 与 `display()`，生成端只发方法调用，不再造第二套转换面。
///
/// 括号包住接收者：`5.display()` 会被词法读成浮点字面量 `5.` 加成员。
/// 没有 `Display` 实现的类型在这里没有兜底（不拼字符串、不回退 `format`）：
/// 调用点以成员解析失败闭合，诊断自带类型名。
fn display_of(expr: &str) -> String {
    format!("({expr}).display()")
}

fn rewrite_utf8_expr_body_guarded(expr: &str, bindings: &[ScriptBinding], loops: &[LoopFrame], visiting: &mut Vec<String>) -> String {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return encode_v_text_literal("");
    }
    if let Some((cond, then_arm, else_arm)) = split_awsl_ternary(trimmed) {
        return format!(
            "(if {} {{ {} }} else {{ {} }})",
            rewrite_expr_with_loops(&cond, bindings, loops),
            rewrite_utf8_expr_body_guarded(&then_arm, bindings, loops, visiting),
            rewrite_utf8_expr_body_guarded(&else_arm, bindings, loops, visiting)
        );
    }
    for binding in bindings {
        if binding.reactive && trimmed == binding.name {
            // `SignalValueType` 是 ABI 已定的通道事实，不是按表达式形状猜类型：
            // `Utf8` 槽本身就是文本，`I32`/`Bool` 才需要 `Display` 投影。
            return match binding.value_type {
                SignalValueType::Utf8 => binding.read_expr(),
                SignalValueType::I32 | SignalValueType::Bool => display_of(&binding.read_expr()),
            };
        }
    }
    // 非响应式绑定（Derived/Memoized/LocalConst）：生成源码没有模块级 `let`，
    // 整体引用直接内联其 init 表达式（递归展开，环引用时保持原名、由编译报错兜底）。
    for binding in bindings {
        if !binding.reactive && !binding.init_expr.trim().is_empty() && trimmed == binding.name && !visiting.iter().any(|v| v == &binding.name)
        {
            visiting.push(binding.name.clone());
            let expanded = rewrite_utf8_expr_body_guarded(&binding.init_expr, bindings, loops, visiting);
            visiting.pop();
            return expanded;
        }
    }
    let out = rewrite_expr_with_loops(trimmed, bindings, loops);
    // 字符串字面量按合法捕获原样输出；其余表达式一律交给类型系统的 `Display` 合同，
    // 生成端不按值形态猜数字/布尔/标识符转换，也不把 `+` 拆成文本拼接。
    if out.starts_with('"') && out.ends_with('"') && out.len() >= 2 {
        return out;
    }
    display_of(&out)
}

pub(crate) fn rewrite_expr_with_loops(expr: &str, bindings: &[ScriptBinding], loops: &[LoopFrame]) -> String {
    rewrite_expr_with_loops_guarded(expr, bindings, loops, &mut Vec::new())
}

fn rewrite_expr_with_loops_guarded(expr: &str, bindings: &[ScriptBinding], loops: &[LoopFrame], visiting: &mut Vec<String>) -> String {
    let mut out = normalize_js_string_literals(strip_expr_braces(expr.trim()));
    for binding in bindings.iter().filter(|b| b.reactive) {
        if loops.iter().any(|f| f.item_var == binding.name || f.index_var == binding.name) {
            continue;
        }
        out = replace_ident(&out, &binding.name, &binding.read_expr());
    }
    // 非响应式绑定：复合表达式中的裸标识符同样内联 init（生成源码没有模块级 `let` 可引用）。
    for binding in bindings.iter().filter(|b| !b.reactive && !b.init_expr.trim().is_empty()) {
        if loops.iter().any(|f| f.item_var == binding.name || f.index_var == binding.name) {
            continue;
        }
        if visiting.iter().any(|v| v == &binding.name) {
            continue;
        }
        visiting.push(binding.name.clone());
        let inlined = rewrite_expr_with_loops_guarded(&binding.init_expr, bindings, loops, visiting);
        visiting.pop();
        out = replace_ident(&out, &binding.name, &format!("({inlined})"));
    }
    normalize_awsl_js_ops(&out)
}

pub(crate) fn normalize_awsl_js_ops(expr: &str) -> String {
    if expr.is_empty() {
        return "true".into();
    }
    normalize_js_string_literals(expr).replace("===", "==").replace("!==", "!=")
}

pub(crate) fn normalize_js_string_literals(expr: &str) -> String {
    let mut out = String::new();
    let mut chars = expr.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            let mut inner = String::new();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        inner.push(next);
                    }
                }
                else if c == '\'' {
                    break;
                }
                else {
                    inner.push(c);
                }
            }
            out.push_str(&encode_v_text_literal(&inner));
        }
        else if ch == '"' {
            out.push(ch);
            while let Some(c) = chars.next() {
                out.push(c);
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                }
                else if c == '"' {
                    break;
                }
            }
        }
        else {
            out.push(ch);
        }
    }
    out
}
