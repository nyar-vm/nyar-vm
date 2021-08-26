use super::*;

#[derive(Debug)]
pub struct ParsingContext {
    pub refine: bool,
    pub file_id: u32,
    errors: Vec<NyarError>,
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
    pub fn push_error(&mut self, error: NyarError) {
        self.errors.push(error)
    }
}