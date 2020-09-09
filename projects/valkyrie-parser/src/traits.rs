use crate::LexerContext;
use nyar_hir::{ASTKind, Result};
use std::{fs::File, io::Write};

pub trait ASTDump {
    fn parse(input: &str) -> Self;
    fn save(&self, path: &str) -> Result<()>;
}

impl ASTDump for ASTKind {
    fn parse(input: &str) -> Self {
        let mut cfg = LexerContext::default();
        cfg.refine = true;
        cfg.get_ast(input).unwrap()
    }
    fn save(&self, path: &str) -> Result<()> {
        let text = format!("{:#?}", self);
        let mut file = File::create(path)?;
        file.write_all(text.as_bytes())?;
        Ok(())
    }
}
