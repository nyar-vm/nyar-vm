use super::*;

use indextree::{Ancestors, Arena, Children, Descendants, Node, NodeId};
use std::{collections::HashSet, iter::FromIterator};

pub struct ModuleManager {
    modules_arena: Arena<ModuleInstance>,
    root_name: Option<String>,
    root_module: NodeId,
    current_module: NodeId,
}

impl Default for ModuleManager {
    fn default() -> Self {
        let mut modules_arena = Arena::new();
        let root = modules_arena.new_node(ModuleInstance::default());
        Self { modules_arena, root_name: None, root_module: root, current_module: root }
    }
}

impl ModuleManager {
    pub fn new(name: &str) -> Self {
        Self { root_name: Some(String::from(name)), ..Self::default() }
    }
    pub fn count(&self) -> usize {
        self.modules_arena.count()
    }
}

impl ModuleManager {
    #[inline]
    fn get(&self, id: NodeId) -> &Node<ModuleInstance> {
        self.modules_arena.get(id).unwrap()
    }
    #[inline]
    fn get_mut(&mut self, id: NodeId) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(id).unwrap()
    }
    #[inline]
    fn get_node_name(&self, id: NodeId) -> Option<String> {
        self.modules_arena.get(id).and_then(|f| f.get().name.to_owned())
    }
    #[inline]
    pub fn get_root_module(&self) -> &Node<ModuleInstance> {
        self.get(self.root_module)
    }
    #[inline]
    pub fn get_root_module_mut(&mut self) -> &mut Node<ModuleInstance> {
        self.get_mut(self.root_module)
    }
    #[inline]
    pub fn get_current_module(&self) -> &Node<ModuleInstance> {
        self.get(self.current_module)
    }
    #[inline]
    pub fn get_current_module_mut(&mut self) -> &mut Node<ModuleInstance> {
        self.get_mut(self.current_module)
    }
    #[inline]
    pub fn get_father_module(&self) -> &Node<ModuleInstance> {
        let id = self.get_current_module().parent().unwrap();
        self.get(id)
    }
    #[inline]
    pub fn get_father_module_mut(&mut self) -> &mut Node<ModuleInstance> {
        let id = self.get_current_module().parent().unwrap();
        self.get_mut(id)
    }
    #[inline]
    pub fn get_children_modules(&self) -> Children<ModuleInstance> {
        self.current_module.children(&self.modules_arena)
    }
    pub fn get_children_names(&self) -> Vec<String> {
        let mut names = vec![];
        for node in self.get_children_modules() {
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
    pub fn get_ancestors_modules(&self) -> Ancestors<ModuleInstance> {
        self.current_module.ancestors(&self.modules_arena)
    }
    pub fn get_full_path(&self) {}
    pub fn get_full_path_name(&self) {}
    #[inline]
    pub fn get_descendants_modules(&self) -> Descendants<ModuleInstance> {
        self.current_module.descendants(&self.modules_arena)
    }
}

impl ModuleManager {
    pub fn new_child_module(&mut self, name: &str) -> Result<()>{
        if self.get_children_names_set().contains(name) {
            return Err(NyarError::msg("submodule already exists"))
        }
        let module = ModuleInstance::new_module(name);
        let id = self.modules_arena.new_node(module);
        self.current_module.append(id, &mut self.modules_arena);
        Ok(())
    }
    pub fn new_child_module_then_switch(&mut self, name: &str) -> Result<()>{
        if self.get_children_names_set().contains(name) {
            return Err(NyarError::msg("submodule already exists"))
        }
        let module = ModuleInstance::new_module(name);
        let id = self.modules_arena.new_node(module);
        self.current_module.append(id, &mut self.modules_arena);
        self.current_module = id;
        Ok(())
    }

    pub fn new_child_scope(&mut self) -> Result<()>{
        let module = ModuleInstance::new_scope();
        let id = self.modules_arena.new_node(module);
        self.current_module.append(id, &mut self.modules_arena);
        Ok(())
    }

    pub fn new_child_scope_then_switch(&mut self) -> Result<()>{
        let module = ModuleInstance::new_scope();
        let id = self.modules_arena.new_node(module);
        self.current_module.append(id, &mut self.modules_arena);
        self.current_module = id;
        Ok(())
    }
}

impl ModuleManager {
    pub fn switch_to_father_module(&mut self) -> Result<()> {
        let id = self.get_current_module().parent().unwrap();
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
        for node in self.get_children_modules() {
            match &self.get_node_name(node) {
                Some(s) if s == name => {
                    self.current_module = node;
                    return Ok(())
                },
                _ => {}
            }
        }
        return Err(NyarError::msg("no such module"))
    }
    pub fn switch_by_path(&mut self, path: &str) -> Result<()> {
        unimplemented!()
    }
}

impl ModuleManager {}

#[test]
fn test() {
    let mut arena = ModuleManager::default();
    arena.new_child_scope();
    arena.new_child_module_then_switch("a1");
    arena.new_child_module_then_switch("a2");
    arena.new_child_module("a3");
    arena.switch_to_root_module();
    arena.new_child_module_then_switch("b1");
    arena.new_child_module("b2");
    arena.switch_to_root_module();
    println!("{:#?}", arena.get_children_names_set())
}
