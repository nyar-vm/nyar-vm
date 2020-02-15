use serde_json;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AST {
    /// - `None`: It doesn't look like anything to me
    None,
    /// - `Program`:
    Program(Vec<AST>),
    /// - `EmptyStatement`: Nothing
    EmptyStatement,

    ImportStatement {
        data: ImportStatement,
        modifier: Annotation,
    },

    ///
    NumberLiteral {
        handler: String,
        data: String,
    },

    ///
    Bytes(Vec<u8>),
    /// - `StringLiteral`: raw string with handler
    StringLiteral {
        handler: String,
        data: String,
    },
    /// - `String`: raw string
    String(String),
    /// - `Comment`: raw comment with handler
    CommentLiteral {
        handler: String,
        data: String,
    },
    ///
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportStatement {
    Local { root: u8, path: Vec<String> },
    LocalAlias { root: u8, path: Vec<String>, alias: String },
    URL { path: String },
    URLAlias { path: String, alias: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Annotation {
    None,
}
