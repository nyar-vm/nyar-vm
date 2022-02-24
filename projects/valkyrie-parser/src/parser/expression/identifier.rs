use valkyrie_ast::ValkyrieIdentifier;

use crate::parser::valkyrie::{Namepath, SpecialNode};

use super::*;

impl IdentifierNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieIdentifier {
        ValkyrieIdentifier::new(self.get_identifier(), parser.file, &self.position)
    }
    pub fn get_identifier(&self) -> String {
        self.string.to_string()
    }
}

impl Namepath {
    pub fn extract(&self) -> Vec<ValkyrieIdentifier> {
        let mut out = vec![];
        for name in &self.path {
            out.push(name.visit(&mut ValkyrieParser::default()))
        }
        out
    }
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        ValkyrieASTNode::namepath(self.extract(), parser.file, &self.position)
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
