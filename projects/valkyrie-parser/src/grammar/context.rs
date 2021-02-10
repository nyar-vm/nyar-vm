use super::*;

#[derive(Debug)]
pub struct ParsingContext {
    pub refine: bool,
    pub file_id: u32,
    pub errors: Vec<NyarError>,
}

impl Default for ParsingContext {
    fn default() -> Self {
        Self { refine: true, file_id: 0, errors: vec![] }
    }
}

impl ParsingContext {
    pub fn new(file_id: u32) -> Self {
        ParsingContext { file_id, ..Default::default() }
    }
    pub fn get_span(&self, s: &Pair<Rule>) -> Span {
        Span { start: s.as_span().start() as u32, end: s.as_span().end() as u32, file_id: self.file_id }
    }
}
