use pest::Span;
use pest::iterators::Pair;
use lsp_types::{Range, Position};
use crate::{ASTNode, ASTKind};

fn get_position<T>(s: &Pair<T>) -> Range {
    let us = s.as_span().start_pos().line_col();
    let es = s.as_span().end_pos().line_col();
    Range {
        // index: s.start_pos().pos() as u64,
        start: Position { line: us.0 as u32, character: us.1 as u32 },
        end: Position { line: es.0 as u32, character: es.1 as u32 },
    }
}

