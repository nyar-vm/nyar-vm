#[derive(Clone, Debug)]
pub enum Pattern {
    Literal(i64),
    Variable(String),
    Constructor(String, Vec<Pattern>),
    Wildcard,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    Variable(String),
    Call(Box<Expr>, Vec<Expr>),
    Add(Box<Expr>, Box<Expr>),
    // Closure: args, body
    Closure(Vec<String>, Vec<Stmt>),
    // OOP
    New(String),
    GetField(Box<Expr>, String),
    SetField(Box<Expr>, String, Box<Expr>),
    InstanceOf(Box<Expr>, String),
    Cast(Box<Expr>, String),
    // Pattern Matching
    Match(Box<Expr>, Vec<(Pattern, Vec<Stmt>)>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Expr(Expr),
    Let(String, Expr),
    Return(Expr),
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
}
