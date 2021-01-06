use nyar_error::Span;

use crate::ast::ASTMeta;
use crate::ASTNode;

pub fn meta_range_from_block(v: &[ASTNode]) -> ASTMeta {
    let head = v.first().unwrap_or_default();
    let tail = v.last().unwrap_or_default();
    ASTMeta {
        span: Span {
            start: head.start(),
            end: tail.end(),
            file_id: head.file_id(),
        },
        document: "".to_string(),
    }
}

