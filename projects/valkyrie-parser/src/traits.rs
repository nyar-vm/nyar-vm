use crate::ParsingContext;
use nyar_hir::{ASTKind, Result};
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
        let mut out = String::new();
        match self {
            ASTKind::Program(v) => {
                for i in v {
                    writeln!(out, "{:#?}", i)?;
                }
            }
            _ => unreachable!(),
        }
        let mut file = File::create(path)?;
        file.write_all(out.as_bytes())?;
        Ok(())
    }
}
