use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub symbol: SymbolNode,
    pub modifiers: Vec<String>,
    pub block: Vec<ASTNode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub symbol: SymbolNode,
    pub modifiers: Vec<String>,
    pub default: Option<ASTNode>,
}

pub enum FunctionParameterKind {
    Normal,
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
