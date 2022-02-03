use valkyrie_ast::{BinaryExpression, ExpressionOrderResolver, OperatorKind, UnaryExpression, UnknownOrder, ValkyrieOperator};

use crate::parser::valkyrie::{ExprNode, ExpressionNode, TermNode};

use super::*;

mod identifier;
mod number;

impl ExpressionNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let mut todo = vec![];
        self.expr.visit(parser, &mut todo)?;
        for term in self.infix {
            let o = ValkyrieOperator::infix(&term.infix.string, parser.file, &term.infix.position)?;
            todo.push(UnknownOrder::Infix(o));
            term.expr.visit(parser, &mut todo)?;
        }
        ExpressionOrderResolver::res(todo.into_iter())?
    }
}

impl ExprNode {
    pub fn visit(&self, parser: &mut ValkyrieParser, terms: &mut Vec<UnknownOrder>) -> ValkyrieResult {
        for prefix in self.prefix {
            let o = ValkyrieOperator::prefix(&prefix.string, parser.file, &prefix.position)?;
            terms.push(UnknownOrder::Prefix(o));
        }
        let term = self.term.visit(parser)?;
        terms.push(UnknownOrder::Value(term));
        for suffix in self.suffix {
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
            TermNode::IdentifierNode(v) => Ok(v.visit(parser).to_node()),
            TermNode::NumberNode(v) => Ok(v.visit(parser)),
            TermNode::StringNode(_) => {
                todo!()
            }
            TermNode::SpecialNode(s) => {
                let out = match s.string.as_str() {
                    "true" => ValkyrieASTNode::boolean(true, parser.file, &s.position),
                    "false" => ValkyrieASTNode::boolean(false, parser.file, &s.position),
                    "null" => ValkyrieASTNode::null(parser.file, &s.position),
                    _ => panic!("Unknown special node: {}", s.string),
                };
                Ok(out)
            }
            TermNode::TupleStatement(v) => {
                let mut out = vec![];
                for term in &v.args {
                    out.push(term.visit(parser)?)
                }
                Ok(ValkyrieASTNode::tuple(out, parser.file, &v.position))
            }
            TermNode::ListStatement(v) => {
                let mut out = vec![];
                for term in &v.args {
                    out.push(term.visit(parser)?)
                }
                Ok(ValkyrieASTNode::list(out, parser.file, &v.position))
            }
        }
    }
}
