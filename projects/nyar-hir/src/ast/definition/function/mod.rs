use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub symbol: SymbolNode,
    pub modifiers: Vec<String>,
    pub parameters: Vec<FunctionParameter>,
    pub block: Vec<ASTNode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub kind: FunctionParameterKind,
    pub symbol: SymbolNode,
    pub modifiers: Vec<String>,
    pub default: Option<ASTNode>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum FunctionParameterKind {
    BothAvailable,
    PositionalOnly,
    NamedOnly,
    Deconstruct2,
    Deconstruct3,
    Receiver,
}

impl Default for FunctionParameterKind {
    fn default() -> Self {
        Self::PositionalOnly
    }
}

impl FunctionDefinition {
    pub fn push_symbol(&mut self, symbol: Symbol, span: Span) {
        self.symbol = SymbolNode(symbol, span)
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::DefineFunction(box self), span }
    }
}

impl FunctionParameter {
    pub fn push_symbol(&mut self, symbol: Symbol, span: Span) {
        self.symbol = SymbolNode(symbol, span)
    }
    pub fn is_delimiter(&self) -> bool {
        self.kind.is_delimiter()
    }
}

impl FunctionParameterKind {
    pub fn is_delimiter(&self) -> bool {
        match self {
            FunctionParameterKind::PositionalOnly | FunctionParameterKind::NamedOnly => true,
            _ => false,
        }
    }
}
