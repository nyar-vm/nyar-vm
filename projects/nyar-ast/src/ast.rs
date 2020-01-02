#[derive(Debug, Clone, PartialEq)]
pub enum AST {
    /// - `None`: It doesn't look like anything to me
    None,
    /// - `Program`:
    Program(Vec<AST>),
    /// - `EmptyStatement`: Nothing
    EmptyStatement,

    ///
    Number { handler: String, data: String },
    ///
    String { handler: String, data: String },
}

#[allow(unused_variables)]
impl AST {
    pub fn new_number(data: &str, handler: &str) {}
    pub fn new_string(data: &str, handler: &str) {}
}
