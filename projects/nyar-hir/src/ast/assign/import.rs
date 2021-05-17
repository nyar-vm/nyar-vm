use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImportStatement {
    /// ```vk
    /// import a
    /// import a as b
    /// ```
    Symbol { name: Symbol, alias: Option<Symbol> },
    /// ```vk
    /// import "path" {a}
    /// import "path" {a as b}
    /// ```
    Script { path: String, name: Symbol, alias: Option<Symbol> },
}

pub struct ImportBuilder {
    path: String,
    name: Option<Symbol>,
    alias: Option<Symbol>,
    block: Vec<ImportBuilder>,
}

impl Default for ImportBuilder {
    fn default() -> Self {
        Self { path: "".to_string(), name: None, alias: None, block: vec![] }
    }
}

impl ImportBuilder {
    pub fn push_string_path(&mut self, path: String) {
        self.path = path;
    }
    pub fn push_symbol_path(&mut self, path: Symbol) {
        self.name = Some(path);
    }
    pub fn push_alias(&mut self, alias: Symbol) {
        self.alias = Some(alias);
    }
    pub fn push_block(&mut self, item: ImportBuilder) {
        self.block.push(item);
    }
    pub fn as_node(&self, span: Span) -> ASTNode {
        ASTNode::null(span)
    }
}
