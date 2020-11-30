mod interpreter;
mod module;

pub use self::module::ModuleInstance;

use crate::{
    engine::{interpreter::Evaluate, module::ModuleManager},
    ASTNode, Result, Value,
};
use std::collections::HashMap;

pub struct NyarEngine {
    pub(crate) import_pkg: HashMap<String, ModuleManager>,
    pub(crate) current_pkg: ModuleManager,
}

impl Default for NyarEngine {
    fn default() -> Self {
        Self { import_pkg: Default::default(), current_pkg: Default::default() }
    }
}

impl NyarEngine {
    pub fn evaluate(&mut self, ast: &ASTNode) -> Result<Value> {
        ast.evaluate(self)
    }
}
