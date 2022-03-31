use nyar_error::third_party::Url;
use std::path::PathBuf;
use valkyrie_ast::{ProgramRoot, StatementKind};

#[salsa::input]
pub struct NyarFile {
    #[return_ref]
    pub path: NyarFileLocation,
    #[return_ref]
    pub contents: String,
}

#[derive(Debug)]
pub enum NyarFileLocation {
    Snippet(String),
    Local(PathBuf),
    Remote(Url),
}

#[salsa::tracked]
pub struct NyarProgram {
    #[return_ref]
    pub statements: Vec<StatementKind>,
}

#[salsa::tracked]
pub struct NyarFunction {
    #[id]
    pub name: FunctionID,
    #[return_ref]
    pub parameters: Vec<ParameterID>,
}

#[salsa::interned]
pub struct FunctionID {
    #[return_ref]
    pub name: Vec<Symbol>,
}

#[salsa::interned]
pub struct ParameterID {
    #[return_ref]
    pub name: Vec<Symbol>,
}

#[salsa::interned]
pub struct Symbol {
    #[return_ref]
    pub name: String,
}
