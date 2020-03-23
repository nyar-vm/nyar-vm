use crate::ast::AST;

#[allow(unused_variables)]
impl AST {
    pub fn parse_string(self) -> AST {
        return match self {
            AST::StringLiteral { handler, data } => AST::String("".to_string()),
            AST::String(s) => AST::String(s),
            _ => AST::String("".to_string()),
        };
    }
    pub fn parse_integer(self) -> AST {
        unimplemented!()
    }
    pub fn save(self, path: &str) {
        let json = serde_json::to_string(&self).unwrap();
        println!("{}", json);
    }
    pub fn load(path: &str) -> AST {
        unimplemented!()
    }
}
