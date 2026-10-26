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

pub(crate) fn escape_v_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('{', "\\{").replace('}', "\\}")
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
            let before_ok = i == 0 || !is_ident_char(chars[i - 1]);
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
