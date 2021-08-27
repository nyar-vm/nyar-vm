use std::{
    fs::read_to_string,
    ops::Range,
    path::{Path, PathBuf},
};

use peginator::PegParser;

use nyar_error::{NamedSource, NyarResult, ParseError, Url};
use nyar_hir::ASTNode;

use crate::{parser::valkyrie::VkParser, ValkyrieParser};

#[allow(non_camel_case_types)]
mod valkyrie;

pub struct ParseContext {
    source: String,
    file_name: String,
    path: PathBuf,
}

impl ValkyrieParser {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> NyarResult<ASTNode> {
        let path = path.as_ref().canonicalize()?;
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let mut ctx = ParseContext { source: read_to_string(&path)?, file_name, path };
        ctx.parse()?;
        Ok(ASTNode::default())
    }
}

impl ParseContext {
    pub fn parse_error(&self, msg: impl Into<String>, span: Range<usize>) -> ParseError {
        let path = Url::from_file_path(&self.path).unwrap();
        let path = path.to_string();
        ParseError { message: msg.into(), file: NamedSource::new(path, self.source.clone()), span }
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
        Ok(())
    }
}
