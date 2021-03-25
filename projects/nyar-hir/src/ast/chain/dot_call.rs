use super::*;

impl AddAssign<Symbol> for ChainCall {
    fn add_assign(&mut self, rhs: Symbol) {
        self.chain.push(CallableItem::DotCall(rhs));
    }
}

impl ChainCall {
    pub fn dot_number_call(&mut self, number: IntegerLiteral, span: Span) {
        let n = ASTNode::number(number, span);
        self.chain.push(CallableItem::SliceCall(SliceArgument { terms: vec![SliceTerm::Index { index: n }] }));
    }

    pub fn static_call(&mut self, rhs: Symbol) {
        debug_assert!(rhs.scope.is_empty());
        self.chain.push(CallableItem::StaticCall(rhs.name));
    }
}
