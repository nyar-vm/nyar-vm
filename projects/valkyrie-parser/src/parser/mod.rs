use std::{ops::Range, path::Path};

use peginator::PegParser;

use nyar_error::{NamedSource, NyarResult, ParseError};
use nyar_hir::ASTNode;

use crate::{parser::valkyrie::VkParser, ValkyrieParser};

#[allow(non_camel_case_types)]
mod valkyrie;

pub struct ParseContext {
    source: String,
    file_name: String,
}

impl ValkyrieParser {
    pub fn parse(input: &str, file_name: &str) -> NyarResult<ASTNode> {
        let mut ctx = ParseContext { source: input.to_string(), file_name: file_name.to_string() };
        ctx.parse()?;
        todo!()
    }
    pub fn parse_file<P: AsRef<Path>>(path: P) -> NyarResult<ASTNode> {
        let path = path.as_ref().canonicalize()?;
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();

        let mut ctx = ParseContext { source: std::fs::read_to_string(path)?, file_name };
        ctx.parse()?;
        todo!()
    }
}

impl ParseContext {
    pub fn parse_error(&self, msg: impl Into<String>, span: Range<usize>) -> ParseError {
        ParseError { message: msg.into(), file: NamedSource::new(self.file_name.clone(), self.source.clone()), url: (), span }
    }
}

impl ParseContext {
    pub fn parse(&mut self) -> NyarResult<()> {
        let stmts = match VkParser::parse(&self.file_name) {
            Ok(o) => o.statements,
            Err(e) => Err(self.parse_error(e.specifics.to_string(), Range { start: e.position, end: e.position }))?,
        };
        for stmt in stmts {
            println!("{:?}", stmt);
        }
        todo!()
    }
}
