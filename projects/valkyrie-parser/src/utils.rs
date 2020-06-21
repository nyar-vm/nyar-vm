use nyar_ast::ast::Position;
use pest::iterators::Pair;
use pest::Span;

fn format_pair(pair: Pair<&str>, indent_level: usize, is_newline: bool) -> String {
    let indent = if is_newline { "  ".repeat(indent_level) } else { "".to_string() };
    let children: Vec<_> = pair.clone().into_inner().collect();
    let len = children.len();
    let children: Vec<_> = children.into_iter().map(|pair| format_pair(pair, if len > 1 { indent_level + 1 } else { indent_level }, len > 1)).collect();
    let dash = if is_newline { "- " } else { "" };
    match len {
        0 => format!("{}{}{}: {:?}", indent, dash, pair.as_rule(), pair.as_span().as_str()),
        1 => format!("{}{}{} > {}", indent, dash, pair.as_rule(), children[0]),
        _ => format!("{}{}{}\n{}", indent, dash, pair.as_rule(), children.join("\n")),
    }
}

pub fn get_position(s: Span) -> Position {
    Position { start: s.start_pos().line_col(), end: s.end_pos().line_col() }
}
