// use super::*;

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
    pub fn get_span(&self) -> Self {
        self.refine = true;
        self
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
