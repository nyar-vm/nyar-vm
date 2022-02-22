use super::*;

mod identifier;
mod number;
mod string;
mod table;

impl ExpressionNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut todo = vec![];
        self.expr.visit(parser, &mut todo)?;
        for term in &self.infix {
            let o = ValkyrieOperator::infix(&term.infix.string, parser.file, &term.infix.position)?;
            todo.push(UnknownOrder::Infix(o));
            term.expr.visit(parser, &mut todo)?;
        }
        ExpressionOrderResolver::resolve(todo)
    }
}

impl ExprNode {
    pub fn visit(&self, parser: &mut ValkyrieParser, terms: &mut Vec<UnknownOrder>) -> ValkyrieResult {
        for prefix in &self.prefix {
            let o = ValkyrieOperator::prefix(&prefix.string, parser.file, &prefix.position)?;
            terms.push(UnknownOrder::Prefix(o));
        }
        let term = self.term.visit(parser)?;
        terms.push(UnknownOrder::Value(term));
        for suffix in &self.suffix {
            let o = ValkyrieOperator::suffix(&suffix.string, parser.file, &suffix.position)?;
            terms.push(UnknownOrder::Suffix(o));
        }
        Ok(())
    }
}

impl TermNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        match self {
            TermNode::ExpressionNode(e) => e.visit(parser),
            TermNode::Namepath(v) => Ok(v.visit(parser)),
            TermNode::NumberNode(v) => {
                let maybe = v.visit(parser);
                parser.safe_node(maybe, &v.position)
            }
            TermNode::StringNode(v) => {
                let maybe = v.visit(parser);
                parser.safe_node(maybe, &v.position)
            }
            TermNode::SpecialNode(v) => Ok(v.visit(parser)),
            TermNode::TableStatement(v) => v.visit(parser),
            TermNode::ListStatement(v) => {
                let mut out = vec![];
                for term in &v.args {
                    out.push(term.visit(parser)?)
                }
                Ok(ValkyrieASTNode::list(out, parser.file, &v.position))
            }
            TermNode::TupleStatement(v) => {
                let mut out = vec![];
                for term in &v.args {
                    out.push(term.visit(parser)?)
                }
                Ok(ValkyrieASTNode::tuple(out, parser.file, &v.position))
            }
        }
    }
}
