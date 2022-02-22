use crate::parser::valkyrie::{KeyNode, KeyValueNode, TableStatement, ValueNode};
use valkyrie_ast::HeterogeneousList;

use super::*;

impl TableStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut pairs = vec![];
        for row in &self.args {
            pairs.push(row.visit(parser)?)
        }
        Ok(ValkyrieASTNode::dict(pairs, parser.file, &self.position))
    }
}

impl KeyValueNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<HeterogeneousList> {
        let key = self.key.visit(parser)?;
        let value = self.value.visit(parser)?;
        Ok(HeterogeneousList::pair(key, value))
    }
}

impl KeyNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        match self {
            KeyNode::IdentifierNode(v) => v.visit(parser),
            KeyNode::StringNode(v) => {
                // v.visit(parser).asstr
                todo!()
            }
        }
        todo!()
    }
}

impl ValueNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        match self {
            ValueNode::IntegerNode(v) => {}
            ValueNode::StringNode(_) => {}
        }
        todo!()
    }
}
