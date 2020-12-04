use super::*;
use crate::{
    engine::{ModuleInstance, ModuleManager, SharedModule},
    value::NyarClass,
};
use std::{rc::Rc, sync::Arc};

#[derive(Debug, Clone)]
pub enum Symbol {
    Alias(AliasSymbol),
    Module(SharedModule),
    Variable,
    Class(Arc<NyarClass>),
}

#[derive(Debug, Clone)]
pub struct AliasSymbol {
    name: String,
    src: Box<Symbol>,
}

impl ModuleManager {
    pub fn find_symbol(&self, path: &[String]) -> Symbol {
        for name in path {
            print!("{}", name)
        }
        Symbol::Module(self.get_current_module().clone())
    }
    pub fn find_symbol_by_path(&self, path: &str) -> Symbol {
        for name in path.split("::") {
            print!("{}", name)
        }
        Symbol::Module(self.get_current_module().clone())
    }
}

#[test]
fn test() {
    let mut pkg = ModuleManager::new("test");
    pkg.new_child_module("a");

    let sym = pkg.find_symbol_by_path("a::b");
    println!("{:#?}", sym)
}
