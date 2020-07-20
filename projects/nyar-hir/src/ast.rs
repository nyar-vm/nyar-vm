use arc_number::Number;

pub struct ASTNode<N> {
    kind: ASTKind<N>,
    range: Position,
}

#[derive(Debug, Clone)]
pub enum ASTKind<T> {
    Program(Vec<ASTKind<T>>),
    Suite(Vec<ASTKind<T>>),
    /// - `EmptyStatement`: Skip
    EmptyStatement,
    /// - `ImportStatement`
    ImportStatement {
        data: ImportStatement,
        annotations: Option<Box<ASTKind<T>>>,
    },
    IfStatement {
        pairs: Vec<(ASTKind<T>, ASTKind<T>)>,
        default: Option<Box<ASTKind<T>>>,
        annotations: Option<Box<ASTKind<T>>>,
    },
    LetBinding {
        symbol: Box<ASTKind<T>>,
        modifiers: Vec<String>,
        types: Box<ASTKind<T>>,
        annotations: Option<Box<ASTKind<T>>>,
    },
    /// - `Expression`
    Expression {
        base: Box<ASTKind<T>>,
        eos: bool,
        pos: Position,
        annotations: Option<Box<ASTKind<T>>>,
    },
    /// - `Expression`
    TypeExpression {
        base: Box<ASTKind<T>>,
        pos: Position,
        annotations: Option<Box<ASTKind<T>>>,
    },
    /// - `UnaryOperators`
    ///     - `base`
    UnaryOperators {
        base: Box<ASTKind<T>>,
        prefix: Vec<String>,
        suffix: Vec<String>,
        pos: Position,
    },
    /// - `InfixOperators`
    InfixOperators {
        o: String,
        lhs: Box<ASTKind<T>>,
        rhs: Box<ASTKind<T>>,
        pos: Position,
    },
    ///
    ListExpression(Vec<ASTKind<T>>),
    ///
    TupleExpression(Vec<ASTKind<T>>),
    /// - `SliceExpression`
    /// the terms must `IndexExpression`
    SliceExpression {
        base: Box<ASTKind<T>>,
        list: Vec<ASTKind<T>>,
    },
    IndexExpression(IndexExpression),
    ApplyExpression {
        base: Box<ASTKind<T>>,
        types: Vec<ASTKind<T>>,
        args: Vec<ASTKind<T>>,
        kv_pairs: Vec<(ASTKind<T>, ASTKind<T>)>,
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
        start: Box<ASTKind>,
        end: Box<ASTKind>,
        step: Box<ASTKind>,
    },
}
