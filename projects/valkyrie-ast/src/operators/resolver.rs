use super::*;

#[derive(Default)]
pub struct ExpressionOrderResolver {}

impl ExpressionOrderResolver {
    pub fn resolve(terms: Vec<UnknownOrder>) -> ValkyrieResult<ValkyrieASTNode> {
        let mut this = ExpressionOrderResolver {};
        Ok(this.parse(terms.into_iter())?)
    }
}

impl OperatorKind {
    pub fn affix(&self) -> Affix {
        match self {
            // suffix

            // prefix
            OperatorKind::Positive => Affix::Prefix(Precedence(20100)),
            OperatorKind::Negative => Affix::Prefix(Precedence(20100)),
            OperatorKind::RootOf(_) => Affix::Prefix(Precedence(20100)),
            OperatorKind::Not => Affix::Prefix(Precedence(20100)),
            // infix power
            OperatorKind::Power => Affix::Infix(Precedence(10200), Right),
            // infix times
            OperatorKind::MultiplyBroadcast => Affix::Infix(Precedence(10100), Right),
            OperatorKind::Slash => Affix::Infix(Precedence(10100), Left),
            // infix add/subtract
            OperatorKind::Add => Affix::Infix(Precedence(10000), Left),
            OperatorKind::Subtract => Affix::Infix(Precedence(10000), Left),
            // infix add/subtract
            OperatorKind::Return => Affix::Infix(Precedence(2), Left),
            OperatorKind::Is(_) => Affix::Infix(Precedence(2), Left),
            OperatorKind::In(_) => Affix::Infix(Precedence(2), Left),
            OperatorKind::Contains(_) => Affix::Infix(Precedence(2), Left),
            OperatorKind::Concat => Affix::Infix(Precedence(2), Left),
            OperatorKind::Greater => Affix::Infix(Precedence(2), Left),
            OperatorKind::GreaterEqual => Affix::Infix(Precedence(2), Left),
            OperatorKind::Less => Affix::Infix(Precedence(2), Left),
            OperatorKind::LessEqual => Affix::Infix(Precedence(2), Left),
        }
    }
}

impl ValkyrieOperator {
    pub fn affix(&self) -> Affix {
        self.kind.affix()
    }
    pub fn is_prefix(&self) -> bool {
        match self.affix() {
            Affix::Prefix(_) => true,
            _ => false,
        }
    }
    pub fn is_infix(&self) -> bool {
        match self.affix() {
            Affix::Infix(_, _) => true,
            _ => false,
        }
    }
    pub fn is_suffix(&self) -> bool {
        match self.affix() {
            Affix::Postfix(_) => true,
            _ => false,
        }
    }
}

impl<I> PrattParser<I> for ExpressionOrderResolver
where
    I: Iterator<Item = UnknownOrder>,
{
    type Error = SyntaxError;
    type Input = UnknownOrder;
    type Output = ValkyrieASTNode;

    fn query(&mut self, tree: &UnknownOrder) -> Result<Affix, SyntaxError> {
        let affix = match tree {
            UnknownOrder::Prefix(o) => o.affix(),
            UnknownOrder::Infix(o) => o.affix(),
            UnknownOrder::Suffix(o) => o.affix(),
            UnknownOrder::Group(_) => Affix::Nilfix,
            UnknownOrder::Value(_) => Affix::Nilfix,
        };
        Ok(affix)
    }

    fn primary(&mut self, tree: UnknownOrder) -> Result<ValkyrieASTNode, SyntaxError> {
        let expr = match tree {
            UnknownOrder::Value(node) => node,
            UnknownOrder::Group(group) => self.parse(&mut group.into_iter()).unwrap(),
            _ => unreachable!(),
        };
        Ok(expr)
    }

    fn infix(
        &mut self,
        lhs: ValkyrieASTNode,
        tree: UnknownOrder,
        rhs: ValkyrieASTNode,
    ) -> Result<ValkyrieASTNode, SyntaxError> {
        match tree {
            UnknownOrder::Infix(o) => Ok(BinaryExpression::combine(lhs, o, rhs)),
            _ => unreachable!(),
        }
    }
    fn prefix(&mut self, tree: UnknownOrder, rhs: ValkyrieASTNode) -> Result<ValkyrieASTNode, SyntaxError> {
        match tree {
            UnknownOrder::Prefix(o) => Ok(UnaryExpression::combine(rhs, o)),
            _ => unreachable!(),
        }
    }
    fn postfix(&mut self, lhs: ValkyrieASTNode, tree: UnknownOrder) -> Result<ValkyrieASTNode, SyntaxError> {
        match tree {
            UnknownOrder::Prefix(o) => Ok(UnaryExpression::combine(lhs, o)),
            _ => unreachable!(),
        }
    }
}
