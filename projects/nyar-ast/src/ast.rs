#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AST {
    Program(Vec<AST>),
    /// - `EmptyStatement`: Skip
    EmptyStatement,
    /// - `ImportStatement`
    ImportStatement {
        data: ImportStatement,
        modifier: Option<Annotation>,
    },
    IfStatement {
        conditions: Vec<AST>,
        blocks: Vec<AST>,
        default: Option<Box<AST>>,
        modifier: Option<Annotation>,
    },
    /// - `UnaryOperators`
    ///     - `base`
    UnaryOperators {
        base: Box<AST>,
        prefix: Vec<String>,
        postfix: Vec<String>,
        modifier: Option<Annotation>,
    },
    /// - `InfixOperators`
    InfixOperators {
        operator: String,
        lhs: Box<AST>,
        rhs: Box<AST>,
        modifier: Option<Annotation>,
    },
    /// - `Symbol`
    Symbol {
        name: String,
        scope: Vec<String>,
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
    /// - `None`: It doesn't look like anything to me
    None,
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
