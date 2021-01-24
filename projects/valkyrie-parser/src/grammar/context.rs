// use super::*;

use nyar_error::Span;
use valkyrie_pest::{Pair, Rule};

#[derive(Debug)]
pub struct ParsingContext {
    pub refine: bool,
    pub file_id: u32,
}

impl Default for ParsingContext {
    fn default() -> Self {
        Self { refine: true, file_id: 0 }
    }
}

impl ParsingContext {
    pub fn new(file_id: u32) -> Self {
        ParsingContext { refine: false, file_id }
    }
    pub fn get_span(&self, s: &Pair<Rule>) -> Span {
        Span { start: s.as_span().start() as u32, end: s.as_span().end() as u32, file_id: self.file_id }
    }
}
