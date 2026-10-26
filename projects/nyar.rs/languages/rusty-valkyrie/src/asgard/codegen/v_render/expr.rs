use crate::asgard::awsl::{ScriptBinding, SignalValueType, expr_util::split_awsl_ternary};

use super::{
    loop_frame::LoopFrame,
    util::{escape_v_string, is_simple_identifier, replace_ident, strip_expr_braces},
};

pub(crate) fn rewrite_utf8_expr_body(expr: &str, bindings: &[ScriptBinding], loops: &[LoopFrame]) -> String {
    let trimmed = expr.trim();
    if let Some(expanded) = expand_awsl_fstring(trimmed) {
        return rewrite_utf8_expr_body(&expanded, bindings, loops);
    }
    if let Some((cond, then_arm, else_arm)) = split_awsl_ternary(trimmed) {
        return format!(
            "(if {} {{ {} }} else {{ {} }})",
            rewrite_expr_with_loops(&cond, bindings, loops),
            rewrite_utf8_expr_body(&then_arm, bindings, loops),
            rewrite_utf8_expr_body(&else_arm, bindings, loops)
        );
    }
    for binding in bindings {
        if binding.reactive && trimmed == binding.name {
            return match binding.value_type {
                SignalValueType::Utf8 => binding.read_expr(),
                SignalValueType::I32 | SignalValueType::Bool => format!("utf8({})", binding.read_expr()),
            };
        }
    }
    let mut out = rewrite_expr_with_loops(trimmed, bindings, loops);
    if out.starts_with('"') && out.ends_with('"') && out.len() >= 2 {
        return out;
    }
    if out.parse::<i32>().is_ok() {
        return format!("utf8({out})");
    }
    if out == "true" || out == "false" {
        return format!("\"{out}\"");
    }
    if is_simple_identifier(&out) {
        return out;
    }
    if is_loop_field_access(&out, loops) {
        return format!("utf8({out})");
    }
    if out.contains('+') {
        return out.split('+').map(|part| rewrite_utf8_expr_body(part.trim(), bindings, loops)).collect::<Vec<_>>().join(" + ");
    }
    if out.contains('(') || out.contains('.') {
        return format!("utf8({})", normalize_awsl_js_ops(&out));
    }
    awsl_expr_to_v(&out)
}

pub(crate) fn rewrite_expr_with_loops(expr: &str, bindings: &[ScriptBinding], loops: &[LoopFrame]) -> String {
    let mut out = normalize_js_string_literals(strip_expr_braces(expr.trim()));
    for binding in bindings.iter().filter(|b| b.reactive) {
        if loops.iter().any(|f| f.item_var == binding.name || f.index_var == binding.name) {
            continue;
        }
        out = replace_ident(&out, &binding.name, &binding.read_expr());
    }
    normalize_awsl_js_ops(&out)
}

fn expand_awsl_fstring(expr: &str) -> Option<String> {
    let trimmed = expr.trim();
    let rest = trimmed.strip_prefix('f')?;
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    if rest.len() < 2 || !rest.ends_with(quote) {
        return None;
    }
    let inner = &rest[1..rest.len() - 1];
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            if !literal.is_empty() {
                parts.push(format!("\"{}\"", escape_v_string(&literal)));
                literal.clear();
            }
            let mut depth = 1usize;
            let mut expr_body = String::new();
            while let Some(c) = chars.next() {
                if c == '{' {
                    depth += 1;
                    expr_body.push(c);
                }
                else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    expr_body.push(c);
                }
                else {
                    expr_body.push(c);
                }
            }
            parts.push(expr_body.trim().to_string());
            continue;
        }
        literal.push(ch);
    }
    if !literal.is_empty() {
        parts.push(format!("\"{}\"", escape_v_string(&literal)));
    }
    if parts.is_empty() {
        return Some("\"\"".into());
    }
    Some(parts.join(" + "))
}

fn is_loop_field_access(expr: &str, loops: &[LoopFrame]) -> bool {
    if let Some((head, _)) = expr.split_once('.') {
        return loops.iter().any(|f| f.item_var == head);
    }
    loops.iter().any(|f| f.item_var == expr || f.index_var == expr)
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
            out.push('"');
            out.push_str(&escape_v_string(&inner));
            out.push('"');
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

fn awsl_expr_to_v(expr: &str) -> String {
    let trimmed = expr.trim();
    if let Some(expanded) = expand_awsl_fstring(trimmed) {
        return awsl_expr_to_v(&expanded);
    }
    let trimmed = normalize_js_string_literals(trimmed);
    if trimmed.is_empty() {
        return "\"\"".into();
    }
    if let Some((cond, then_arm, else_arm)) = split_awsl_ternary(&trimmed) {
        return format!("if {} {{ {} }} else {{ {} }}", awsl_expr_to_v(&cond), awsl_expr_to_v(&then_arm), awsl_expr_to_v(&else_arm));
    }
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        let inner = &trimmed[1..trimmed.len() - 1];
        return format!("\"{}\"", escape_v_string(inner));
    }
    if trimmed.parse::<i32>().is_ok() {
        return trimmed;
    }
    if trimmed == "true" || trimmed == "false" {
        return format!("\"{trimmed}\"");
    }
    if is_simple_identifier(&trimmed) {
        return trimmed;
    }
    format!("utf8({})", normalize_awsl_js_ops(&trimmed))
}
