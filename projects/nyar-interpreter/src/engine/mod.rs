mod interpreter;
mod module;

pub use self::module::ModuleInstance;

use crate::{engine::interpreter::Evaluate, ASTNode, Result, Value};
use std::collections::HashMap;

use indextree::Arena;

#[test]
fn test() {
    // Create a new arena
    let arena = &mut Arena::new();

// Add some new nodes to the arena
    let a = arena.new_node(1);
    let b = arena.new_node(2);


// Append b to a
    a.append(b, arena);
    assert_eq!(b.ancestors(arena).into_iter().count(), 2);
}


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
