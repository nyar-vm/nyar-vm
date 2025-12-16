#[derive(Clone, Debug)]
pub enum Pattern {
    Literal(i64),
    LiteralString(String),
    Variable(String),
    Constructor(String, Vec<Pattern>),
    Wildcard,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    String(String),
    Bool(bool),
    Variable(String),
    Call(Box<Expr>, Vec<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    // Logical
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    // Comparison
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    // Unary
    Neg(Box<Expr>),
    TypeOf(Box<Expr>),
    // Closure: args, body
    Closure(Vec<String>, Vec<Stmt>),
    // OOP
    New(String),
    GetField(Box<Expr>, String),
    SetField(Box<Expr>, String, Box<Expr>),
    SetLocal(String, Box<Expr>),
    InstanceOf(Box<Expr>, String),
    Cast(Box<Expr>, String),
    CheckCast(Box<Expr>, String),
    // Pattern Matching
    Match(Box<Expr>, Vec<(Pattern, Vec<Stmt>)>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Expr(Expr),
    Let(String, Expr),
    Return(Expr),
    Yield(Expr),
    Assert(Expr, Option<Expr>),
    Debug(Expr),
    NamespaceSet(Vec<String>),
    NamespaceDef(String, Vec<Stmt>),
    Using(Vec<String>),
    // micro name(args) { body }
    FuncDef(String, Vec<String>, Vec<Stmt>),
    // class Name { field1, field2 }
    ClassDef(String, Vec<String>),
    // enum Name { Variant1, Variant2(f1, f2) }
    // Vec<(VariantName, Fields)>
    EnumDef(String, Vec<(String, Vec<String>)>),
    // trait Name { method1, method2 }
    TraitDef(String, Vec<String>),
    // impl Trait for Class { methods }
    ImplDef(String, String, Vec<Stmt>),
    // imply ClassPath { micro method(args) { body }* }
    ImplyDef(String, Vec<Stmt>),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    Loop(Vec<Stmt>),
    Break,
    Continue,
    Line(u32),
}
