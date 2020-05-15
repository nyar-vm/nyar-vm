use crate::ast::{Number, AST};
use std::{fs::File, io::Write};

#[allow(unused_variables)]
impl AST {
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).unwrap();
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
    pub fn load(path: &str) -> AST {
        unimplemented!()
    }
    pub fn parse_number(&self) -> AST {
        let ast = self.clone();
        match ast {
            AST::NumberLiteral { .. } => Number::parse_number(ast),
            _ => ast,
        }
    }
    pub fn parse_string(&self) -> AST {
        let ast = self.clone();
        return match self {
            AST::StringLiteral { handler, data } => match handler.as_str() {
                "" => AST::String(data.clone()),
                _ => ast,
            },
            _ => ast,
        };
    }
}
