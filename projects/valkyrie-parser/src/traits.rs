use crate::ParsingContext;
use nyar_error::Result;
use nyar_hir::ASTKind;
use std::{fmt::Write as _, fs::File, io::Write};

pub trait ASTDump {
    fn parse(input: &str) -> Self;
    fn save(&self, path: &str) -> Result<()>;
}

impl ASTDump for ASTKind {
    fn parse(input: &str) -> Self {
        let mut cfg = ParsingContext::default();
        cfg.refine = true;
        cfg.get_ast(input).unwrap()
    }
    fn save(&self, path: &str) -> Result<()> {
        let mut out = self.pretty_print(9999)?;
        let mut file = File::create(path)?;
        file.write_all(out.as_bytes())?;
        Ok(())
    }
}
