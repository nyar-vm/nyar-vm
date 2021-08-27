use std::ops::Range;

use peginator::PegParser;

use nyar_error::{NamedSource, ParseError, Result};
use nyar_hir::ASTNode;

use crate::{parser::valkyrie::VkParser, ValkyrieParser};

#[allow(non_camel_case_types)]
mod valkyrie;

pub struct ParseContext {
    source: String,
    file_name: String,
}

impl ValkyrieParser {
    pub fn parse(input: &str, file_name: &str) -> Result<ASTNode> {
        let mut ctx = ParseContext { source: input.to_string(), file_name: file_name.to_string() };
        ctx.parse()?;
        todo!()
    }
}

impl ParseContext {
    pub fn parse_error(&self, msg: impl Into<String>, span: Range<usize>) -> ParseError {
        ParseError::new(msg.into(), NamedSource::new(self.file_name.clone(), self.source.clone()), span)
    }
}

impl ParseContext {
    pub fn parse(&mut self) -> Result<()> {
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

#[test]
fn test() -> Result<()> {
    let mut out = ValkyrieParser::parse("fn main() { println!(\"Hello, world!\"); }", "test.vk")?;

    Ok(())
}
