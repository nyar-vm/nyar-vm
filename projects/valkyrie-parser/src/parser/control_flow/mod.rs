use valkyrie_ast::{ForLoop, ValkyriePattern, WhileLoop};

use super::*;

impl ForStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let condition = self.value.visit(parser)?;
        let pattern = ValkyriePattern {};
        let mut this = ForLoop::new(pattern, condition);
        for statement in &self.body.statements {
            statement.visit(parser, this.mut_body())?
        }
        Ok(this.to_node(parser.file, &self.position))
    }
}

impl LoopStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let placeholder = Range { start: self.position.start, end: self.position.start + 4 };
        let condition = ValkyrieASTNode::boolean(true, parser.file, &placeholder);
        let mut this = WhileLoop::new(condition);
        for statement in &self.body.statements {
            statement.visit(parser, this.mut_body())?
        }
        Ok(this.to_node(parser.file, &self.position))
    }
}

impl WhileStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let condition = self.condition.visit(parser)?;
        let mut this = WhileLoop::new(condition);
        for statement in &self.body.statements {
            statement.visit(parser, this.mut_body())?
        }
        Ok(this.to_node(parser.file, &self.position))
    }
}
