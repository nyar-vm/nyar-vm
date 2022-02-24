use crate::parser::valkyrie::IntegerNode;

use super::*;

impl NumberNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let hint = match &self.hint {
            Some(s) => Some(s.as_identifier(parser)),
            None => None,
        };
        match &self.variant {
            NumberVariant::IntegerNode(v) => v.visit(parser),
            NumberVariant::DecimalNode(v) => ValkyrieASTNode::decimal(&v.string, parser.file, &v.position, hint),
            NumberVariant::ByteBin(v) => ValkyrieASTNode::binary(&v.string, parser.file, &v.position),
            NumberVariant::ByteHex(v) => ValkyrieASTNode::hex(&v.string, parser.file, &v.position),
        }
    }
}

impl IntegerNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        ValkyrieASTNode::integer(&self.string, parser.file, &self.position, None)
    }
}
