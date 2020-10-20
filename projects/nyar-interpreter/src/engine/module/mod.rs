mod context;

pub use self::context::{NyarContext, NyarIndexSystem, NYAR_CONTEXT_PRESET};
use crate::value::{function::NyarFunction, variable::Variable};
use std::{collections::HashMap, lazy::SyncLazy, rc::Weak};
use std::cell::RefCell;
use std::rc::Rc;
use crate::Result;
use nyar_hir::{NyarError, Range};

#[derive(Clone, Debug)]
pub struct ScopeInstance {
    pub name: Option<String>,
    father: Option<Weak<Self>>,
    // children: Vec<Rc<Self>>,
    context: NyarContext,
    variable_table: HashMap<String, Variable>,
}

impl ScopeInstance {
    pub fn root() -> Rc<Self> {
        Rc::new(Self {
            name: None,
            father: None,
            context: Default::default(),
            variable_table: Default::default(),
        })
    }
    pub fn fork(&self, name: Option<String>) -> Rc<Self> {
        Rc::new_cyclic(|self_weak| Self {
            name,
            father: Some(self_weak.clone()),
            context: Default::default(),
            variable_table: Default::default(),
        })
    }
}

impl ScopeInstance {
    pub fn get_variable(&self, name: &str, r: Option<Range>) -> Result<&Variable> {
        match self.variable_table.get(name) {
            Some(s) => {Ok(s)}
            None => {

                Err(NyarError::variable_not_found(name, r ))
            }
        }
    }

    pub fn set_variable(&mut self, name: String, value: Variable, r: Option<Range>)-> Result<()> {
        if let Some(v) = self.variable_table.get(&name) {
            if !v.is_mutable() {
                return Err(NyarError::write_unwritable_object(&name, r))
            }
        }
        self.variable_table.insert(name, value);
        Ok(())
    }
}
