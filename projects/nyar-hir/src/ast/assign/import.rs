use super::*;

#[derive(Debug, Clone)]
pub enum ImportStatement {
    /// assign a
    Symbol(String),
    /// assign a as b
    SymbolAlias(Vec<String>, String),
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

pub enum ImportBuilder {}
