mod settings;

use nyar_hir::{Position, Range};
use valkyrie_pest::{Pair, Rule};

pub fn format_pair(pair: Pair<&str>, indent_level: usize, is_newline: bool) -> String {
    let indent = if is_newline { "  ".repeat(indent_level) } else { "".to_string() };
    let children: Vec<_> = pair.clone().into_inner().collect();
    let len = children.len();
    let children: Vec<_> = children
        .into_iter()
        .map(|pair| format_pair(pair, if len > 1 { indent_level + 1 } else { indent_level }, len > 1))
        .collect();
    let dash = if is_newline { "- " } else { "" };
    match len {
        0 => format!("{}{}{}: {:?}", indent, dash, pair.as_rule(), pair.as_span().as_str()),
        1 => format!("{}{}{} > {}", indent, dash, pair.as_rule(), children[0]),
        _ => format!("{}{}{}\n{}", indent, dash, pair.as_rule(), children.join("\n")),
    }
}

pub fn get_position(s: &Pair<Rule>) -> Range {
    let us = s.as_span().start_pos().line_col();
    let es = s.as_span().end_pos().line_col();
    Range {
        // index: s.start_pos().pos() as u64,
        start: Position { line: us.0 as u32, character: us.1 as u32 },
        end: Position { line: es.0 as u32, character: es.1 as u32 },
    }
}

pub fn unescape(s: &str) -> &str {
    return s;
}
