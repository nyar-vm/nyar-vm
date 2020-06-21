use crate::ast::{Number, Position, AST};
use std::{fs::File, io::Write};

pub trait Dump<T> {
    fn save(&self, path: &str) -> std::io::Result<()>;
    fn load(path: &str) -> T;
}

pub trait Refine<T> {
    fn parse_number(&self) -> T;
    fn parse_string(&self) -> T;
}

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
    pub fn set_base(self, replace: AST) -> AST {
        match self {
            AST::ApplyExpression { base, types, args, kv_pairs, pos } => {
                return AST::ApplyExpression { base: Box::new(replace), types, args, kv_pairs, pos };
            }
            _ => self,
        }
    }
}
