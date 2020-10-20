mod interpreter;
mod module;

pub use self::module::ScopeInstance;

use std::collections::HashMap;

pub struct NyarEngine {
    deps_module: HashMap<String, ScopeInstance>,
    root_module: ScopeInstance,
}

