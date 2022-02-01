use super::*;
use crate::BinaryExpression;

#[derive(Debug)]
pub enum BinOpKind {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Pow, // ^
    Eq,  // =
}

#[derive(Debug)]
pub enum UnOp {
    Not, // !
    Neg, // -
    Try, // ?
}

impl ValkyrieOperator {
    pub fn affix(&self) -> Affix {
        match self {
            ValkyrieOperator::Add => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::Subtract => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::MultiplyBroadcast => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::Slash => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::Return => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::Is(_) => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::In(_) => Affix::Infix(Precedence(2), Neither),
            ValkyrieOperator::Contains(_) => Affix::Infix(Precedence(2), Neither),
        }
    }
}

#[allow(unused_variables)]
impl<I> PrattParser<I> for ExpressionOrderResolve
where
    I: Iterator<Item = UnknownOrder>,
{
    type Error = SyntaxError;
    type Input = UnknownOrder;
    type Output = ValkyrieASTNode;

    fn query(&mut self, tree: &UnknownOrder) -> ValkyrieResult<Affix> {
        let affix = match tree {
            UnknownOrder::Prefix(o) => o.affix(),
            UnknownOrder::Infix(o) => o.affix(),
            UnknownOrder::Suffix(o) => o.affix(),
            UnknownOrder::Group(_) => Affix::Nilfix,
            UnknownOrder::Value(_) => Affix::Nilfix,
        };
        Ok(affix)
    }

    fn primary(&mut self, tree: UnknownOrder) -> ValkyrieResult<ValkyrieASTNode> {
        let expr = match tree {
            UnknownOrder::Value(node) => node,
            UnknownOrder::Group(group) => self.parse(&mut group.into_iter()).unwrap(),
            _ => unreachable!(),
        };
        Ok(expr)
    }

    // Construct a binary infix expression, e.g. 1+1
    fn infix(&mut self, lhs: ValkyrieASTNode, tree: UnknownOrder, rhs: ValkyrieASTNode) -> ValkyrieResult<ValkyrieASTNode> {
        match tree {
            UnknownOrder::Infix(op) => Ok(BinaryExpression::combine(lhs, op, rhs)),
            _ => unreachable!(),
        }
    }
    fn prefix(&mut self, tree: UnknownOrder, rhs: ValkyrieASTNode) -> ValkyrieResult<ValkyrieASTNode> {
        let op = match tree {
            UnknownOrder::Infix('+') => BinOpKind::Add,
            UnknownOrder::Infix('-') => BinOpKind::Sub,
            UnknownOrder::Infix('*') => BinOpKind::Mul,
            UnknownOrder::Infix('/') => BinOpKind::Div,
            UnknownOrder::Infix('^') => BinOpKind::Pow,
            UnknownOrder::Infix('=') => BinOpKind::Eq,
            _ => unreachable!(),
        };
        Ok(ValkyrieASTNode::BinOp(Box::new(lhs), op, Box::new(rhs)))
    }
    fn postfix(&mut self, lhs: ValkyrieASTNode, tree: UnknownOrder) -> ValkyrieResult<ValkyrieASTNode> {
        let op = match tree {
            UnknownOrder::Infix('+') => BinOpKind::Add,
            _ => unreachable!(),
        };
        Ok(ValkyrieASTNode::BinOp(Box::new(lhs), op, Box::new(rhs)))
    }
}
