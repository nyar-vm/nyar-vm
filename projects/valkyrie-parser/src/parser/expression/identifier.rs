use valkyrie_ast::ValkyrieIdentifier;

use crate::parser::valkyrie::{Namepath, SpecialNode};

use super::*;

impl IdentifierNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        self.as_identifier(parser).to_node(parser.file, &self.position)
    }
    pub fn as_identifier(&self, parser: &mut ValkyrieParser) -> ValkyrieIdentifier {
        ValkyrieIdentifier::new(self.as_text(), parser.file, &self.position)
    }
    pub fn as_text(&self) -> String {
        self.string.to_string()
    }
}

impl Namepath {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        ValkyrieASTNode::namepath(self.as_path(), parser.file, &self.position)
    }
    pub fn as_path(&self) -> Vec<ValkyrieIdentifier> {
        let mut out = vec![];
        for name in &self.path {
            out.push(name.as_identifier(&mut ValkyrieParser::default()))
        }
        out
    }
}

impl SpecialNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        let out = match self.string.as_str() {
            "true" => ValkyrieASTNode::boolean(true, parser.file, &self.position),
            "false" => ValkyrieASTNode::boolean(false, parser.file, &self.position),
            "null" => ValkyrieASTNode::null(parser.file, &self.position),
            _ => panic!("Unknown special node: {}", self.string),
        };
        out
    }
}
