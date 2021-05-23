use super::*;

pub struct DefineFunction {
    pub symbol: SymbolNode,
}

pub struct DefineBuilder {
    symbol: SymbolNode,
    block: Vec<ASTNode>,
}

impl Default for DefineBuilder {
    fn default() -> Self {
        DefineBuilder { symbol: Default::default(), block: vec![] }
    }
}

impl DefineBuilder {
    pub fn push_symbol(&mut self, symbol: Symbol, span: Span) {
        self.symbol = SymbolNode(symbol, span)
    }
    pub fn push_block(&mut self, block: Vec<ASTNode>, span: Span) {
        self.block = block;
    }
    pub fn as_node(&self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::Define(box self.clone()), span }
    }
}
