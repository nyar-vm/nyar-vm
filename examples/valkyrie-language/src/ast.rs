#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    Variable(String),
    Call(String, Vec<Expr>),
    Add(Box<Expr>, Box<Expr>),
    // Closure: args, body
    Closure(Vec<String>, Vec<Stmt>),
    // OOP
    New(String),
    GetField(Box<Expr>, String),
    SetField(Box<Expr>, String, Box<Expr>),
    InstanceOf(Box<Expr>, String),
    Cast(Box<Expr>, String),
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
}
