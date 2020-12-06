use super::*;

use crate::{value::Symbol, Value};

#[derive(Clone, Debug)]
pub struct ModuleInstance {
    pub name: Option<String>,
    pub context: Option<Box<NyarContext>>,
    pub symbol_table: HashMap<String, Symbol>,
}

impl Default for ModuleInstance {
    fn default() -> Self {
        Self { name: None, context: Default::default(), symbol_table: Default::default() }
    }
}

unsafe impl GcSafe for ModuleInstance {}

unsafe impl GcDrop for ModuleInstance {}

unsafe impl Scan for ModuleInstance {
    fn scan(&self, scanner: &mut shredder::Scanner<'_>) {
        scanner.scan(&self.name);
        check_gc_drop(&self.name);
        // scanner.scan(&self.context);
        // shredder::plumbing::check_gc_drop(__binding_0);
        scanner.scan(&self.symbol_table);
        check_gc_drop(&self.symbol_table);
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
