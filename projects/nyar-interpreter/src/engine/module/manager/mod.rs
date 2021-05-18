use super::*;

use crate::{engine::NyarEngine, value::Symbol};
use indextree::{Ancestors, Arena, Children, Descendants, Node, NodeId};
use std::{
    collections::{BTreeMap, HashSet},
    iter::FromIterator,
    sync::Arc,
};

impl Default for PackageManager {
    fn default() -> Self {
        Self { arena: Default::default() }
    }
}

impl PackageManager {
    pub const THIS_PACKAGE: &'static str = "depot";

    pub fn new<S>(name: S) -> Self
    where
        S: Into<String>,
    {
        Self { root_name: Some(name.into), ..Self::default() }
    }
    #[inline]
    pub fn get_package_name(&self) -> &str {
        match &self.root_name {
            None => THIS_PACKAGE,
            Some(s) => s.as_str(),
        }
    }
    #[inline]
    pub fn count(&self) -> usize {
        self.arena.count()
    }
}

impl PackageManager {
    #[inline]
    fn get(&self, id: NodeId) -> &Node<SharedModule> {
        self.arena.get(id).unwrap()
    }
    #[inline]
    fn get_mut(&mut self, id: NodeId) -> &mut Node<SharedModule> {
        self.arena.get_mut(id).unwrap()
    }
    #[inline]
    fn get_node_name(&self, id: NodeId) -> Option<String> {
        self.arena.get(id).and_then(|f| f.get().read().ok()).and_then(|f| f.name.to_owned())
    }
    #[inline]
    pub fn get_root_module(&self) -> &SharedModule {
        self.get(self.root_module).get()
    }
    // #[inline]
    // pub fn get_root_module_mut(&mut self) -> &mut SharedModule {
    //     self.get_mut(self.root_module).get_mut()
    // }
    #[inline]
    pub fn get_current_module(&self) -> &SharedModule {
        self.get(self.current_module).get()
    }
    // #[inline]
    // pub fn get_current_module_mut(&mut self) -> &mut SharedModule {
    //     self.get_mut(self.current_module).get_mut()
    // }
    #[inline]
    pub fn get_parent_id(&self) -> NodeId {
        self.get(self.current_module).parent().unwrap()
    }
    #[inline]
    pub fn get_parent_module(&self) -> &SharedModule {
        self.get(self.get_parent_id()).get()
    }
    // #[inline]
    // pub fn get_parent_module_mut(&mut self) -> &mut SharedModule {
    //     self.get_mut(self.get_parent_id()).get_mut()
    // }
    #[inline]
    fn get_children_id(&self) -> Children<SharedModule> {
        self.current_module.children(&self.arena)
    }
    #[inline]
    pub fn get_children_modules(&self) -> Vec<&SharedModule> {
        self.get_children_id().map(|id| self.get(id).get()).collect()
    }
    pub fn get_children_names(&self) -> Vec<String> {
        let mut names = vec![];
        for node in self.get_children_id() {
            // notice no names means scope
            if let Some(s) = self.get_node_name(node) {
                names.push(s)
            }
        }
        return names;
    }
    #[inline]
    pub fn get_children_names_set(&self) -> HashSet<String> {
        HashSet::from_iter(self.get_children_names().iter().cloned())
    }
    #[inline]
    fn get_ancestors_modules_id(&self) -> Ancestors<SharedModule> {
        self.current_module.ancestors(&self.arena)
    }
    #[inline]
    pub fn get_ancestors_modules(&self) -> Vec<&SharedModule> {
        self.get_ancestors_modules_id().map(|id| self.get(id).get()).collect()
    }
    pub fn get_full_path(&self) {}
    pub fn get_full_path_name(&self) {}
    #[inline]
    fn get_descendants_modules_id(&self) -> Descendants<SharedModule> {
        self.current_module.descendants(&self.arena)
    }
}

impl PackageManager {
    pub fn new_child_module(&mut self, name: &str) -> Result<()> {
        if self.get_children_names_set().contains(name) {
            return Err(NyarError::msg("submodule already exists"));
        }
        let module = ModuleInstance::new_module(name);
        let id = self.arena.new_node(Gc::new(RwLock::new(module)));
        self.current_module.append(id, &mut self.arena);
        Ok(())
    }
    pub fn new_child_module_then_switch(&mut self, name: &str) -> Result<()> {
        if self.get_children_names_set().contains(name) {
            return Err(NyarError::msg("submodule already exists"));
        }
        let module = ModuleInstance::new_module(name);
        let id = self.arena.new_node(Gc::new(RwLock::new(module)));
        self.current_module.append(id, &mut self.arena);
        self.current_module = id;
        Ok(())
    }

    pub fn new_child_scope(&mut self) -> Result<()> {
        let module = ModuleInstance::new_scope();
        let id = self.arena.new_node(Gc::new(RwLock::new(module)));
        self.current_module.append(id, &mut self.arena);
        Ok(())
    }

    pub fn new_child_scope_then_switch(&mut self) -> Result<()> {
        let module = ModuleInstance::new_scope();
        let id = self.arena.new_node(Gc::new(RwLock::new(module)));
        self.current_module.append(id, &mut self.arena);
        self.current_module = id;
        Ok(())
    }
}

impl PackageManager {
    pub fn switch_to_parent_module(&mut self) -> Result<()> {
        let id = self.get_parent_id();
        if self.current_module == self.root_module {
            return Err(NyarError::msg("root module has no father module!"));
        }
        self.current_module = id;
        Ok(())
    }
    pub fn switch_to_root_module(&mut self) -> Result<()> {
        self.current_module = self.root_module;
        Ok(())
    }
    pub fn switch_to_child_module(&mut self, name: &str) -> Result<()> {
        for node in self.get_children_id() {
            match &self.get_node_name(node) {
                Some(s) if s.as_str() == name => {
                    self.current_module = node;
                    return Ok(());
                }
                _ => {}
            }
        }
        return Err(NyarError::msg("no such module"));
    }
    pub fn switch_by_path(&mut self, path: &str) -> Result<()> {
        let _root = self.arena.get(self.root_module).unwrap();
        for _i in path.split("::") {
            unimplemented!()
        }

        unimplemented!()
    }
}

impl PackageManager {}

impl NyarEngine {
    #[inline]
    pub fn new_child_module(&mut self, name: &str) -> Result<()> {
        self.current_pkg.new_child_module(name)
    }
    #[inline]
    pub fn new_child_module_then_switch(&mut self, name: &str) -> Result<()> {
        self.current_pkg.new_child_module_then_switch(name)
    }
    #[inline]
    pub fn new_child_scope(&mut self) -> Result<()> {
        self.current_pkg.new_child_scope()
    }
    #[inline]
    pub fn new_child_scope_then_switch(&mut self) -> Result<()> {
        self.current_pkg.new_child_scope_then_switch()
    }
    #[inline]
    pub fn switch_to_root_module(&mut self) -> Result<()> {
        self.current_pkg.switch_to_root_module()
    }
}
