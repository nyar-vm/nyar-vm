use crate::{parser::valkyrie::ClassStatement, ValkyrieParser};
use valkyrie_ast::{ClassDeclare, ValkyrieASTNode};
use valkyrie_errors::ValkyrieResult;

impl ClassStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut this = ClassDeclare::default();
        *this.mut_namepath() = self.namespace.extract();

        for s in &self.body.statements {
            s.visit(parser, &mut this.mut_statement())?;
        }
        Ok(this.to_node(parser.file, &self.position))
    }
}
