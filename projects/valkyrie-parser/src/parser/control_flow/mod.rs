use super::*;

impl ForStatement {}

impl LoopStatement {}

impl WhileStatement {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut out = While::new(&self.kw);
        out.set_condition(self.condition.visit(parser)?);
        out.set_body(self.body.visit(parser)?);
        Ok(out.to_node(parser.file, &self.position))
    }
}
