use super::*;

impl NumberNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let hint = match &self.hint {
            Some(s) => Some(s.visit(parser)),
            None => None,
        };
        match &self.variant {
            NumberVariant::IntegerNode(v) => ValkyrieASTNode::integer(&v.string, parser.file, &v.position, hint),
            NumberVariant::DecimalNode(v) => ValkyrieASTNode::decimal(&v.string, parser.file, &v.position, hint),
            NumberVariant::ByteNode(v) => {
                println!("ByteNode: {:?}", v);
                todo!()
            }
        }
    }
}
