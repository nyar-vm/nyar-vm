use crate::ast::AST;
use std::{fs::File, io::Write};

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

    pub fn save(self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self).unwrap();
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
    pub fn load(path: &str) -> AST {
        unimplemented!()
    }
}
