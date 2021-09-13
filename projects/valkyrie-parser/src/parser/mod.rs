use std::{ops::Range, };

use peginator::PegParser;

use valkyrie_ast::{NamespaceDeclare, ValkyrieASTNode};
use valkyrie_errors::{FileID, FileSpan, SyntaxError, ValkyrieError, ValkyrieResult};

use crate::{
    parser::valkyrie::{IdentifierNode, LetStatement, NamespaceDeclareNode, VkParser, VkStatements},
    ValkyrieParser,
};

mod expression;
#[allow(non_camel_case_types)]
mod valkyrie;

impl ValkyrieParser {
    pub fn parse_file(&mut self, file: FileID, text: &str) -> ValkyrieResult<Vec<ValkyrieASTNode>> {
        self.file = file;
        let stmts = match VkParser::parse(text) {
            Ok(o) => o.statements,
            Err(e) => Err(SyntaxError::from(e).with_file(file))?,
        };
        let mut out = vec![];
        for s in stmts {
            s.visit(self, &mut out)?;
        }
        Ok(out)
    }
    pub fn take_errors(&mut self) -> Vec<ValkyrieError> {
        std::mem::take(&mut self.errors)
    }
    pub fn push_error(&mut self, message: impl Into<String>, span: &Range<usize>) {
        let error = ValkyrieError::syntax_error(message.into(), FileSpan { file: self.file, head: span.start, tail: span.end });
        self.errors.push(error);
    }
    pub fn bad_node(&self, span: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode::null(self.file, span)
    }
}

impl VkStatements {
    pub fn visit(&self, parser: &mut ValkyrieParser, out: &mut Vec<ValkyrieASTNode>) -> ValkyrieResult {
        match self {
            VkStatements::Semicolon(_) => {}
            VkStatements::ControlFlowNode(_) => {
                todo!()
            }
            VkStatements::DefStatement(_) => {
                todo!()
            }
            VkStatements::ExpressionNode(v) => out.push(v.visit(parser)?),
            VkStatements::LetStatement(v) => out.push(v.visit(parser)?),
            VkStatements::LoopStatement(_) => {
                todo!()
            }
            VkStatements::NamespaceDeclareNode(v) => out.push(v.visit(parser)?),

            VkStatements::WhileStatement(_) => {
                todo!()
            }
        }
        Ok(())
    }
}

impl NamespaceDeclareNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut out = NamespaceDeclare::new(&self.kw);
        for name in &self.namespace.path {
            out.push_name(name.get_identifier());
        }
        Ok(out.to_node(parser.file, &self.namespace.position))
    }
}

impl LetStatement {
    pub fn visit(&self, _parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        todo!()
    }
}

impl ValkyrieParser {}
