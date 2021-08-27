mod gc;
mod interpreter;
pub mod module;

pub use self::module::{ModuleInstance, PackageManager, SharedModule};

use crate::{engine::interpreter::Evaluate, ASTNode, Result, Value};
use std::{collections::HashMap, sync::Arc};

pub struct NyarEngine {
    pub(crate) import_pkg: HashMap<String, Arc<PackageManager>>,
    pub(crate) current_pkg: PackageManager,
    pub(crate) runtime_stack: Vec<Value>,
}

impl Default for NyarEngine {
    fn default() -> Self {
        Self { import_pkg: Default::default(), current_pkg: Default::default(), runtime_stack: vec![] }
    }
}

impl NyarEngine {
    pub fn evaluate(&mut self, ast: &ASTNode) -> Result<Value> {
        ast.evaluate(self)
    }
    pub fn goto(&mut self) {}
    pub fn shift() {}
    pub fn reset() {}
}
