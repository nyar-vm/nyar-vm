mod context;
mod manager;

pub use self::context::{DefaultDecimalHandler, DefaultIntegerHandler, NyarContext, NyarIndexSystem, NYAR_CONTEXT_PRESET};
use crate::{
    value::{function::NyarFunction, variable::Variable},
    Result,
};
use nyar_hir::{NyarError, Range};
use std::{
    collections::HashMap,
    lazy::SyncLazy,
    rc::{Rc, Weak},
};

#[derive(Clone, Debug)]
pub struct ModuleInstance {
    pub name: Option<String>,
    father: Option<Weak<Self>>,
    children: HashMap<String, Rc<Self>>,
    context: NyarContext,
    variable_table: HashMap<String, Variable>,
}

impl Default for ModuleInstance {
    fn default() -> Self {
        Self {
            name: None,
            father: None,
            children: Default::default(),
            context: Default::default(),
            variable_table: Default::default(),
        }
    }
}

impl ModuleInstance {
    pub fn new_module(name:&str) -> Self {
        Self {
            name: Some(String::from(name)),
            father: None,
            children: Default::default(),
            context: Default::default(),
            variable_table: Default::default()
        }
    }
    pub fn new_scope() -> Self {
        Self::default()
    }

    pub fn root() -> Rc<Self> {
        Rc::new(Self::default())
    }
    pub fn fork_scope(&self) -> Rc<Self> {
        Rc::new_cyclic(|self_weak| Self {
            name: None,
            father: Some(self_weak.clone()),
            children: Default::default(),
            context: Default::default(),
            variable_table: Default::default(),
        })
    }
    pub fn fork_module(&mut self, name: String) -> Rc<Self> {
        let sub = Rc::new_cyclic(|self_weak| Self {
            name: Some(name.clone()),
            father: Some(self_weak.clone()),
            children: Default::default(),
            context: Default::default(),
            variable_table: Default::default(),
        });
        self.children.insert(name.clone(), sub);
        self.children.get(&name).unwrap().clone()
    }
}

impl ModuleInstance {
    pub fn get_variable(&self, name: &str, r: Option<Range>) -> Result<Variable> {
        match self.variable_table.get(name) {
            Some(s) => Ok(s.to_owned()),
            None => {
                if let Some(w) = &self.father.as_ref().and_then(|m| m.upgrade()) {
                    return w.get_variable(name, r);
                }
                Err(NyarError::variable_not_found(name, r))
            }
        }
    }

    pub fn set_variable(&mut self, name: String, value: Variable, r: Option<Range>) -> Result<()> {
        if let Some(v) = self.variable_table.get(&name) {
            if !v.is_mutable() {
                return Err(NyarError::write_unwritable_object(&name, r));
            }
        }
        self.variable_table.insert(name, value);
        Ok(())
    }
}
