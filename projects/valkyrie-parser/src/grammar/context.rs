use super::*;

#[derive(Debug)]
pub struct LexerContext {
    pub refine: bool,
}

impl Default for LexerContext {
    fn default() -> Self {
        Self { refine: true }
    }
}