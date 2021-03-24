use super::*;

impl AddAssign<Symbol> for ChainCall {
    fn add_assign(&mut self, rhs: Symbol) {
        self.chain.push(CallableItem::DotSymbolCall(rhs));
    }
}

impl AddAssign<IntegerLiteral> for ChainCall {
    fn add_assign(&mut self, rhs: IntegerLiteral) {
        self.chain.push(CallableItem::DotNumberCall(rhs.value));
    }
}

impl ChainCall {
    pub fn static_call(&mut self, rhs: Symbol) {
        debug_assert!(rhs.scope.is_empty());
        self.chain.push(CallableItem::StaticCall(rhs.name));
    }
}
