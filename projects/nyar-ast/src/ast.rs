use serde_json;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AST {
    /// - `None`: It doesn't look like anything to me
    None,
    /// - `Program`:
    Program(Vec<AST>),
    /// - `EmptyStatement`: Nothing
    EmptyStatement,
    ///
    NumberLiteral { handler: String, data: String },

    ///
    Bytes(Vec<u8>),
    /// - `StringLiteral`: raw string with handler
    StringLiteral { handler: String, data: String },
    /// - `String`: raw string
    String(String),
    /// - `Comment`: raw comment with handler
    Comment { handler: String, data: String },
    ///
    Boolean(bool),
}

#[allow(unused_variables)]
impl AST {
    pub fn new_number(data: &str, handler: &str) {}
    pub fn new_string(data: &str, handler: &str) {}
    pub fn save(self, path: &str) {
        let json = serde_json::to_string(&self).unwrap();
        println!("{}", json);
    }
    pub fn load(path: &str) -> AST {
        unimplemented!()
    }
}
