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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FunctionParameterKind {
    Normal,
    DelimiterPositional,
    DelimiterNamed,
    Deconstruct2,
    Deconstruct3,
    Receiver,
}

impl Default for FunctionParameterKind {
    fn default() -> Self {
        Self::DelimiterPositional
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
    pub fn push_special(&mut self, symbol: &str, span: Span) {
        match symbol {
            "self" => self.symbol = SymbolNode(Symbol::atom("self"), span),
            "<" => self.kind = FunctionParameterKind::DelimiterPositional,
            ">" => self.kind = FunctionParameterKind::DelimiterNamed,
            _ => {}
        }
    }
    pub fn is_delimiter(&self) -> bool {
        match self.kind {
            FunctionParameterKind::DelimiterPositional | FunctionParameterKind::DelimiterNamed => true,
            _ => false,
        }
    }
}
