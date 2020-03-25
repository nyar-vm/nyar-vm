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
    /// - `UnaryOperators`
    ///     - `base`
    ///     - `stack`: (op, isPrefix): (String, bool)
    UnaryOperators {
        base: Box<AST>,
        stack: Vec<(String, bool)>,
    },
    /// - `InfixOperators`
    InfixOperators {
        lhs: Box<AST>,
        rhs: Box<AST>,
        operator: String,
    },
    /// - `Number`: raw number represent
    NumberLiteral {
        handler: String,
        data: String,
    },
    ///
    Number(Number),
    ///
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Number {
    Integer(String),
    Integer8(i8),
    Integer16(i16),
    Integer32(i32),
    Integer64(i64),
    Integer128(i128),
    Integer256(String),
    Unsigned(String),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
    Unsigned128(u128),
    Unsigned256(String),
    Decimal(String),
    Decimal32(f32),
    Decimal64(f64),
}
