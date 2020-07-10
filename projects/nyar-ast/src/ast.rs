use arc_number::Number;

#[derive(Debug, Clone)]
pub enum AST {
    Program(Vec<AST>),
    Suite(Vec<AST>),
    /// - `EmptyStatement`: Skip
    EmptyStatement,
    /// - `ImportStatement`
    ImportStatement {
        data: ImportStatement,
        annotations: Option<Box<AST>>,
    },
    IfStatement {
        pairs: Vec<(AST, AST)>,
        default: Option<Box<AST>>,
        annotations: Option<Box<AST>>,
    },
    LetBinding {
        symbol: Box<AST>,
        modifiers: Vec<String>,
        types: Box<AST>,
        annotations: Option<Box<AST>>,
    },
    /// - `Expression`
    Expression {
        base: Box<AST>,
        eos: bool,
        pos: Position,
        annotations: Option<Box<AST>>,
    },
    /// - `Expression`
    TypeExpression {
        base: Box<AST>,
        pos: Position,
        annotations: Option<Box<AST>>,
    },
    /// - `UnaryOperators`
    ///     - `base`
    UnaryOperators {
        base: Box<AST>,
        prefix: Vec<String>,
        suffix: Vec<String>,
        pos: Position,
    },
    /// - `InfixOperators`
    InfixOperators {
        o: String,
        lhs: Box<AST>,
        rhs: Box<AST>,
        pos: Position,
    },
    ///
    ListExpression(Vec<AST>),
    ///
    TupleExpression(Vec<AST>),
    /// - `SliceExpression`
    /// the terms must `IndexExpression`
    SliceExpression {
        base: Box<AST>,
        list: Vec<AST>,
    },
    IndexExpression(IndexExpression),
    ApplyExpression {
        base: Box<AST>,
        types: Vec<AST>,
        args: Vec<AST>,
        kv_pairs: Vec<(AST, AST)>,
        pos: Position,
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

#[derive(Copy, Clone)]
pub struct Position {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

#[derive(Debug, Clone)]
pub enum ImportStatement {
    /// import a
    Symbol(String),
    /// import a as b
    SymbolAlias(Vec<str>, String),
    Local {
        root: u8,
        path: Vec<String>,
    },
    LocalAlias {
        root: u8,
        path: Vec<String>,
        alias: String,
    },
    URL {
        path: String,
    },
    URLAlias {
        path: String,
        alias: String,
    },
}

#[derive(Debug, Clone)]
pub enum IndexExpression {
    None,
    ///
    Single(),
    Normal {
        start: Box<AST>,
        end: Box<AST>,
        step: Box<AST>,
    },
}
