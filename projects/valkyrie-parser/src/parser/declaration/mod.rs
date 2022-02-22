use crate::{parser::valkyrie::ClassStatement, ValkyrieParser};
use valkyrie_ast::ValkyrieASTNode;
use valkyrie_errors::ValkyrieResult;

impl ClassStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut out = vec![];
        for s in &self.body.statements {
            s.visit(parser, &mut out)?;
        }
        println!("{:?}", out);
        todo!()

        // Ok(ValkyrieASTNode::ClassDeclareNode(ClassDeclareNode {
        //     file: parser.file,
        //     head: self.head,
        //     tail: self.tail,
        //     name: self.name.visit(parser)?,
        //     extends: self.extends.visit(parser)?,
        //     implements: self.implements.visit(parser)?,
        //     statements: out,
        // }))
    }
}
