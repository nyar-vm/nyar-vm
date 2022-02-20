use crate::parser::valkyrie::Namepath;
use valkyrie_ast::ValkyrieIdentifier;

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
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        let mut out = String::new();
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                out.push('.');
            }
            out.push_str(&item.get_identifier());
        }
        ValkyrieIdentifier::new(out, parser.file, &self.position)
    }
}
