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
            OperatorKind::Bang => Affix::Prefix(Precedence(30100)),
            OperatorKind::Question => Affix::Prefix(Precedence(30100)),
            // suffix
            OperatorKind::DivideByDecimalPower(_) => Affix::Prefix(Precedence(30000)),
            OperatorKind::Celsius => Affix::Prefix(Precedence(30000)),
            OperatorKind::Fahrenheit => Affix::Prefix(Precedence(30000)),
            // prefix
            OperatorKind::Positive => Affix::Prefix(Precedence(20000)),
            OperatorKind::Negative => Affix::Prefix(Precedence(20000)),
            OperatorKind::RootOf(_) => Affix::Prefix(Precedence(20000)),
            OperatorKind::Not => Affix::Prefix(Precedence(20000)),
            OperatorKind::Flip => Affix::Prefix(Precedence(20000)),
            OperatorKind::Reverse => Affix::Prefix(Precedence(20000)),
            // infix as: 2 ^ 1 as f64 as i64
            OperatorKind::As => Affix::Infix(Precedence(9000), Left),
            // infix power
            OperatorKind::Power => Affix::Infix(Precedence(10200), Right),
            // infix times: 1 * 2 ^ 3
            OperatorKind::MultiplyBroadcast => Affix::Infix(Precedence(10100), Left),
            OperatorKind::Divide => Affix::Infix(Precedence(10100), Left),
            OperatorKind::Quotient => Affix::Infix(Precedence(10100), Left),
            // infix add/subtract: 1 + 2 * 3
            OperatorKind::Add => Affix::Infix(Precedence(10000), Left),
            OperatorKind::Subtract => Affix::Infix(Precedence(10000), Left),
            // infix bitwise: 1 >> 2 + 3
            // 9900
            // infix comparison: 3 + 1 > 2 + 1
            OperatorKind::Greater => Affix::Infix(Precedence(9800), Left),
            OperatorKind::GreaterEqual => Affix::Infix(Precedence(9800), Left),
            OperatorKind::Less => Affix::Infix(Precedence(9800), Left),
            OperatorKind::LessEqual => Affix::Infix(Precedence(9800), Left),
            // infix logic: 1 > 2 && 3 > 4
            OperatorKind::LogicGate(_) => Affix::Infix(Precedence(9700), Left),
            // infix concat: 1 + 2 ++ [3]
            OperatorKind::Concat => Affix::Infix(Precedence(9600), Left),
            // infix belong: 1 in [1, 2, 3]
            OperatorKind::In(_) => Affix::Infix(Precedence(9500), Left),
            OperatorKind::Contains(_) => Affix::Infix(Precedence(9500), Left),
            // infix check is
            OperatorKind::Is(_) => Affix::Infix(Precedence(9000), Left),
            // infix assignment a = 2 || 3
            OperatorKind::Return => Affix::Infix(Precedence(9000), Left),
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
