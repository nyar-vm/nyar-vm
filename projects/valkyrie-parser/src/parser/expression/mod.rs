use valkyrie_ast::{BinaryExpression, UnaryExpression};

use crate::parser::valkyrie::{ExpressionNode, ExprNode, TermNode};

use super::*;

mod identifier;
mod number;

impl ExpressionNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let lhs = self.expr.visit(parser)?;
        if self.infix.is_empty() {
            return Ok(lhs);
        } else {
            let binary = BinaryExpression {};
            Ok(binary.to_node(parser.file, &Range::default()))
        }
    }
}

impl ExprNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        if self.prefix.is_empty() && self.suffix.is_empty() {
            // must automic
            self.term.visit(parser)
        } else {
            let unary = UnaryExpression {};
            Ok(unary.to_node(parser.file, &Range::default()))
        }
    }
}

impl TermNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        match self {
            TermNode::ExpressionNode(e) => {
                e.visit(parser)
            }
            TermNode::IdentifierNode(v) => Ok(v.visit(parser).to_node()),
            TermNode::NumberNode(v) => {
                Ok(v.visit(parser))
            }
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
            TermNode::MatrixStatement(v) => {
                for arg in &v.args {
                    for ar in &arg.args {
                        println!("{:?}", ar.visit(parser)?);
                    }
                }
                todo!()
            }
        }
    }
}
