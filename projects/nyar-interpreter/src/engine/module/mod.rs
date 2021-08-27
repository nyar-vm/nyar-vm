#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
mod flags;

mod context;
mod instance;
mod manager;
mod optimized;

pub use self::{
    context::{DefaultDecimalHandler, IntegerHandlerManager, NyarContext, NyarIndexSystem, NYAR_CONTEXT_PRESET},
    flags::NyarReadWrite,
    instance::ModuleInstance,
    manager::PackageManager,
};
use crate::{
    value::{function::FunctionPrototype, variable::Variable, Symbol},
    NyarError, Result,
};
use shredder::{
    marker::{GcDrop, GcSafe},
    plumbing::check_gc_drop,
    GRwLock, Gc, Scan, Scanner,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock},
};

/// Module Manager of a package
pub struct PackageManager {
    arena: BTreeMap<Symbol, ModulePolymer>,
}

impl PackageManager {
    pub fn polymer(&mut self) -> Result<()> {
        for (_, module) in self.arena.iter_mut() {
            module.polymer()?;
        }
        Ok(())
    }
}

impl ModulePolymer {
    pub fn polymer(self) -> Result<Self> {
        match self {
            ModulePolymer::Dynamic { primary, provider } => {}
            ModulePolymer::Static { cache } => Ok(Self::Static { cache }),
        }
    }
}

impl ModuleInstance {
    fn polymer_with(&self, primary: &ModulePolymer) -> Self {}
}

///
pub enum ModulePolymer {
    /// A module that is not yet loaded.
    Dynamic { primary: ModuleInstance, provider: Vec<ModuleInstance> },
    /// Readonly module cache, all symbols are resolved.
    Static { cache: Arc<ModuleInstance> },
}

#[derive(Clone, Debug)]
pub struct ModuleInstance {
    is_primary: bool,
    namespace: Option<Symbol>,
    file: Option<String>,
    context: NyarContext,
    symbol_table: HashMap<String, Symbol>,
}
