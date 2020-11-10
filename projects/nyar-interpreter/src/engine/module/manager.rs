use super::*;

use indextree::{Arena, NodeId, Node};

pub struct ModuleManager {
    modules_arena: Arena<ModuleInstance>,
    root_module: NodeId,
    current_module: NodeId,
}

impl Default for ModuleManager {
    fn default() -> Self {
        let mut modules_arena = Arena::new();
        let root = modules_arena.new_node(ModuleInstance::default());
        Self {
            modules_arena,
            root_module: root,
            current_module: root,
        }
    }
}

impl ModuleManager {
    fn get(&self, id: NodeId) -> &Node<ModuleInstance> {
        self.modules_arena.get(id).unwrap()
    }
    fn get_mut(&mut self, id: NodeId) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(id).unwrap()
    }
    pub fn get_root_module(&self) -> &Node<ModuleInstance> {
        self.modules_arena.get(self.root_module).unwrap()
    }
    pub fn get_root_module_mut(&mut self) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(self.root_module).unwrap()
    }
    pub fn get_current_module(&self) -> &Node<ModuleInstance> {
        self.modules_arena.get(self.current_module).unwrap()
    }
    pub fn get_current_module_mut(&mut self) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(self.current_module).unwrap()
    }
    pub fn get_father_module(&self) -> &Node<ModuleInstance> {
        let id = self.get_current_module().parent().unwrap();
        self.modules_arena.get(id).unwrap()
    }
    pub fn get_father_module_mut(&mut self) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(self.current_module).unwrap()
    }

    pub fn new_child_module(&mut self, _name: &str) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(self.current_module).unwrap()
    }

    pub fn new_child_module_switch(&mut self) -> &mut Node<ModuleInstance> {
        self.modules_arena.get_mut(self.current_module).unwrap()
    }

    pub fn switch_to_father_module(&mut self) -> Result<()> {
        let id = self.get_current_module().parent().unwrap();
        if self.current_module == self.root_module {
            return Err(NyarError::msg("root module has no father module!"))
        }
        self.current_module = id;
        Ok(())
    }
    pub fn switch_to_root_module(&mut self) -> Result<()> {
        self.current_module = self.root_module;
        Ok(())
    }
    pub fn switch_to_child_module(&mut self, _name: &str) -> Result<()> {
        self.current_module = self.root_module;
        Ok(())
    }
}


impl ModuleManager {}


#[test]
fn test() {
    // Create a new arena
    let mut arena = ModuleManager::default();

// Add some new nodes to the arena
    let a = arena.new_node(1);
    let b = arena.new_node(2);


// Append b to a
    a.append(b, arena);
    assert_eq!(b.ancestors(arena).into_iter().count(), 2);
}