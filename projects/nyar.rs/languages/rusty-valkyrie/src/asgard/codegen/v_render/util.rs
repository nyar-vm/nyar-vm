pub(crate) fn sanitize_export(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase()
}

pub(crate) fn map_awsl_component_to_host(tag: &str) -> Option<&'static str> {
    match tag {
        "Link" => Some("a"),
        "Badge" => Some("span"),
        "Avatar" => Some("span"),
        "Input" => Some("input"),
        "Button" => Some("button"),
        _ => None,
    }
}

/// 生成合法定界的 V 文本捕获（§4.10 A/D）：定界符只取内容首尾都不是的引号种类，
/// 内部同类引号用更长的引号 run 容纳，`{`/`}`/`\` 与控制字符按捕获解码集编码。
///
/// 首尾同时被两种引号占用时单个捕获无法表达：产出等价的 `+` 拼接表达式，并以空文本
/// 字面量锚定左操作数，让单引号定界的片段按文本上下文解码。绝不用反斜杠转义定界符，
/// 也不 panic。
pub(crate) fn encode_v_text_literal(text: &str) -> String {
    use crate::compiler::hir::lowering::quote_literal::encode_text_capture;
    if let Ok(literal) = encode_text_capture(text) {
        return literal;
    }
    let fragments = encode_capture_fragments(text);
    let anchored = fragments.first().is_some_and(|literal| literal.starts_with('\''));
    if anchored { format!("\"\" + {}", fragments.join(" + ")) } else { fragments.join(" + ") }
}

/// 逐前缀切分：单字符总能选到相反引号作定界符，剩余文本一旦可单捕获即收尾。
fn encode_capture_fragments(text: &str) -> Vec<String> {
    use crate::compiler::hir::lowering::quote_literal::encode_text_capture;
    let mut fragments = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Ok(literal) = encode_text_capture(rest) {
            fragments.push(literal);
            break;
        }
        let first = rest.chars().next().expect("rest is non-empty");
        let mut buffer = [0u8; 4];
        let literal = encode_text_capture(first.encode_utf8(&mut buffer)).expect("single character always has an opposite-quote delimiter");
        fragments.push(literal);
        rest = &rest[first.len_utf8()..];
    }
    fragments
}

pub(crate) fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

pub(crate) fn strip_expr_braces(expr: &str) -> &str {
    let trimmed = expr.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 { trimmed[1..trimmed.len() - 1].trim() } else { trimmed }
}

pub(crate) fn is_simple_identifier(expr: &str) -> bool {
    !expr.is_empty() && expr.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

pub(crate) fn replace_ident(expr: &str, name: &str, replacement: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = expr.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i..].starts_with(&name_chars) {
            // `.` 前缀说明这是字段访问（`post.count`），不是绑定变量引用。
            let before_ok = i == 0 || (chars[i - 1] != '.' && !is_ident_char(chars[i - 1]));
            let after = i + name_chars.len();
            let after_ok = after >= chars.len() || !is_ident_char(chars[after]);
            if before_ok && after_ok {
                out.push_str(replacement);
                i = after;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}
