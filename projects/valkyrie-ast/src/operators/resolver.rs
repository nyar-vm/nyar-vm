use super::*;

// From this
#[derive(Debug)]
pub enum ExpressionUnknownOrder {
    Infix(String),
    Prefix(String),
    Suffix(String),
    Value(ValkyrieASTNode),
    Group(Vec<ExpressionUnknownOrder>),
}

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

pub struct ExpressionOrderResolve {}

#[allow(unused_variables)]
impl<I> PrattParser<I> for ExpressionOrderResolve
where
    I: Iterator<Item = ExpressionUnknownOrder>,
{
    type Error = SyntaxError;
    type Input = ExpressionUnknownOrder;
    type Output = ValkyrieASTNode;

    fn query(&mut self, tree: &ExpressionUnknownOrder) -> ValkyrieResult<Affix> {
        let affix = match tree {
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Infix(Precedence(2), Associativity::Neither),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Infix(Precedence(3), Associativity::Left),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Infix(Precedence(3), Associativity::Left),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Infix(Precedence(4), Associativity::Left),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Infix(Precedence(4), Associativity::Left),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Postfix(Precedence(5)),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Prefix(Precedence(6)),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Prefix(Precedence(6)),
            ExpressionUnknownOrder::Infix(ValkyrieOperator::Add) => Affix::Infix(Precedence(7), Associativity::Right),
            ExpressionUnknownOrder::Group(_) => Affix::Nilfix,
            ExpressionUnknownOrder::Value(_) => Affix::Nilfix,
            _ => unreachable!(),
        };
        Ok(affix)
    }

    // Construct a primary expression, e.g. a number
    fn primary(&mut self, tree: ExpressionUnknownOrder) -> ValkyrieResult<ValkyrieASTNode> {
        let expr = match tree {
            ExpressionUnknownOrder::Value(atom) => atom,
            ExpressionUnknownOrder::Group(group) => self.parse(&mut group.into_iter()).unwrap(),
            _ => unreachable!(),
        };
        Ok(expr)
    }

    // Construct a binary infix expression, e.g. 1+1
    fn infix(
        &mut self,
        lhs: ValkyrieASTNode,
        tree: ExpressionUnknownOrder,
        rhs: ValkyrieASTNode,
    ) -> ValkyrieResult<ValkyrieASTNode> {
        let op = match tree {
            ExpressionUnknownOrder::Infix('+') => BinOpKind::Add,
            ExpressionUnknownOrder::Infix('-') => BinOpKind::Sub,
            ExpressionUnknownOrder::Infix('*') => BinOpKind::Mul,
            ExpressionUnknownOrder::Infix('/') => BinOpKind::Div,
            ExpressionUnknownOrder::Infix('^') => BinOpKind::Pow,
            ExpressionUnknownOrder::Infix('=') => BinOpKind::Eq,
            _ => unreachable!(),
        };
        Ok(ValkyrieASTNode::BinOp(Box::new(lhs), op, Box::new(rhs)))
    }
    fn prefix(&mut self, tree: ExpressionUnknownOrder, rhs: ValkyrieASTNode) -> ValkyrieResult<ValkyrieASTNode> {
        let op = match tree {
            ExpressionUnknownOrder::Infix('+') => BinOpKind::Add,
            ExpressionUnknownOrder::Infix('-') => BinOpKind::Sub,
            ExpressionUnknownOrder::Infix('*') => BinOpKind::Mul,
            ExpressionUnknownOrder::Infix('/') => BinOpKind::Div,
            ExpressionUnknownOrder::Infix('^') => BinOpKind::Pow,
            ExpressionUnknownOrder::Infix('=') => BinOpKind::Eq,
            _ => unreachable!(),
        };
        Ok(ValkyrieASTNode::BinOp(Box::new(lhs), op, Box::new(rhs)))
    }
    fn postfix(&mut self, lhs: ValkyrieASTNode, tree: ExpressionUnknownOrder) -> ValkyrieResult<ValkyrieASTNode> {
        let op = match tree {
            ExpressionUnknownOrder::Infix('+') => BinOpKind::Add,
            ExpressionUnknownOrder::Infix('-') => BinOpKind::Sub,
            ExpressionUnknownOrder::Infix('*') => BinOpKind::Mul,
            ExpressionUnknownOrder::Infix('/') => BinOpKind::Div,
            ExpressionUnknownOrder::Infix('^') => BinOpKind::Pow,
            ExpressionUnknownOrder::Infix('=') => BinOpKind::Eq,
            _ => unreachable!(),
        };
        Ok(ValkyrieASTNode::BinOp(Box::new(lhs), op, Box::new(rhs)))
    }
}
