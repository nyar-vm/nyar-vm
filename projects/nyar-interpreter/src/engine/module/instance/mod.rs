use super::*;

use crate::{value::Symbol, Value};
use std::{
    collections::HashSet,
    lazy::SyncLazy,
    rc::{Rc, Weak},
};

#[derive(Clone, Debug)]
pub struct ModuleInstance {
    pub name: Option<String>,
    // pub context: Option<Box<NyarContext>>,
    pub context: NyarContext,
    pub namespace: HashSet<String>,
    pub variable_table: HashMap<String, Symbol>,
    pub class_table: HashMap<String, Value>,
    pub enum_table: HashMap<String, Value>,
    pub flag_table: HashMap<String, Value>,
}

impl Default for ModuleInstance {
    fn default() -> Self {
        Self {
            name: None,
            context: Default::default(),
            namespace: Default::default(),
            variable_table: Default::default(),
            class_table: Default::default(),
            enum_table: Default::default(),
            flag_table: Default::default(),
        }
    }
}

impl ModuleInstance {
    pub fn new_module(name: &str) -> Self {
        Self { name: Some(String::from(name)), ..Self::default() }
    }
    pub fn new_scope() -> Self {
        Self::default()
    }
    // pub fn root() -> Rc<Self> {
    //     Rc::new(Self::default())
    // }
    // pub fn fork_scope(&self) -> Rc<Self> {
    //     Rc::new_cyclic(|self_weak| Self {
    //         name: None,
    //         context: Default::default(),
    //         variable_table: Default::default(),
    //     })
    // }
    // pub fn fork_module(&mut self, name: String) -> Rc<Self> {
    //     let sub = Rc::new_cyclic(|self_weak| Self {
    //         name: Some(name.clone()),
    //         context: Default::default(),
    //         variable_table: Default::default(),
    //     });
    //     self.children.insert(name.clone(), sub);
    //     self.children.get(&name).unwrap().clone()
    // }
}

impl ModuleInstance {
    pub fn get_variable(&self, name: &str, r: Option<Range>) -> Result<Variable> {
        unimplemented!()
        // match self.variable_table.get(name) {
        //     Some(s) => Ok(s.to_owned()),
        //     None => {
        //         if let Some(w) = &self.father.as_ref().and_then(|m| m.upgrade()) {
        //             return w.get_variable(name, r);
        //         }
        //         Err(NyarError::variable_not_found(name, r))
        //     }
        // }
    }

    pub fn set_variable(&mut self, name: String, value: Variable, r: Option<Range>) -> Result<()> {
        unimplemented!()
        // if let Some(v) = self.variable_table.get(&name) {
        //     if !v.is_mutable() {
        //         return Err(NyarError::write_unwritable_object(&name, r));
        //     }
        // }
        // self.variable_table.insert(name, value);
        // Ok(())
    }
}
