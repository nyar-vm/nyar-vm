use valkyrie_ast::ValkyrieIdentifierNode;

use super::*;

impl IdentifierNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieIdentifierNode {
        ValkyrieIdentifierNode::new(self.get_identifier(), parser.file, &self.position)
    }
    pub fn get_identifier(&self) -> String {
        self.string.to_string()
    }
}
