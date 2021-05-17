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

#[derive(Clone, Debug)]
pub struct ImportBuilder {
    path: String,
    symbol: Symbol,
    alias: Option<Symbol>,
    block: Vec<ImportBuilder>,
}

impl Default for ImportBuilder {
    fn default() -> Self {
        Self { path: "".to_string(), symbol: Default::default(), alias: None, block: vec![] }
    }
}

impl ImportBuilder {
    pub fn push_string_path(&mut self, path: String) {
        self.path = path;
    }
    pub fn push_string_node(&mut self, path: ASTNode) {
        match path.kind {
            ASTKind::String(v) => {
                self.path = v.literal;
            }
            ASTKind::StringTemplate(_) => {
                debug_assert!(false)
            }
            _ => {
                debug_assert!(false)
            }
        }
    }
    pub fn push_symbol_path(&mut self, path: Symbol) {
        self.symbol = path;
    }
    pub fn push_alias(&mut self, alias: Symbol) {
        self.alias = Some(alias);
    }
    pub fn push_block(&mut self, items: Vec<ImportBuilder>) {
        self.block = items;
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> Vec<ASTNode> {
        let (parent, nodes) = self.detach();
        nodes
            .into_iter()
            .map(|f| f.concat_child(&parent))
            .flatten()
            .map(ImportStatement::from)
            .map(|f| f.as_node(span))
            .collect()
    }
    fn detach(self) -> (ImportBuilder, Vec<ImportBuilder>) {
        let ImportBuilder { path, symbol: name, alias, block } = self;
        (ImportBuilder { path, symbol: name, alias, block: vec![] }, block)
    }
    fn concat_child(self, parent: &Self) -> Vec<ImportBuilder> {
        let (this, block) = self.detach();
        let parent = Self {
            //
            path: parent.path.clone(),
            symbol: parent.symbol.concat(this.symbol),
            alias: this.alias,
            block: vec![],
        };
        if block.is_empty() {
            return vec![parent];
        }
        block.into_iter().map(|f| f.concat_child(&parent)).flatten().collect()
    }
}

impl From<ImportBuilder> for ImportStatement {
    fn from(builder: ImportBuilder) -> Self {
        debug_assert!(builder.block.is_empty());
        if builder.path.is_empty() {
            return ImportStatement::Symbol { name: builder.symbol, alias: builder.alias };
        }
        ImportStatement::Script { path: builder.path, name: builder.symbol, alias: builder.alias }
    }
}

impl ImportStatement {
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::ImportStatement(box self), span }
    }
}
