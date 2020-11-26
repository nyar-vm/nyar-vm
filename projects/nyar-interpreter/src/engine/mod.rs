mod interpreter;
mod module;

pub use self::module::ModuleInstance;

use crate::{engine::interpreter::Evaluate, ASTNode, Result, Value};
use std::collections::HashMap;

pub struct NyarEngine {
    deps_module: HashMap<String, ModuleInstance>,
    root_module: ModuleInstance,
}

impl Default for NyarEngine {
    fn default() -> Self {
        Self { deps_module: Default::default(), root_module: Default::default() }
    }
}

impl NyarEngine {
    pub fn evaluate(&mut self, ast: &ASTNode) -> Result<Value> {
        ast.evaluate(self)
    }
}
