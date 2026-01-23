use crate::vm::value::Value;
use crate::vm::VmError;
use std::collections::HashMap;

pub type FFIResult = Result<Value, VmError>;

pub trait FFIFunction: Send + Sync {
    fn call(&self, args: Vec<Value>) -> FFIResult;
}

pub struct FFIRegistry {
    pub functions: HashMap<String, Box<dyn FFIFunction>>,
    pub loaders: HashMap<String, Box<dyn ModuleLoader>>,
}

pub trait ModuleLoader: Send + Sync {
    fn load(&self, path: &str) -> Result<Vec<(String, Box<dyn FFIFunction>)>, String>;
}

impl Default for FFIRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FFIRegistry {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            loaders: HashMap::new(),
        }
    }

    pub fn register_loader(&mut self, name: String, loader: Box<dyn ModuleLoader>) {
        self.loaders.insert(name, loader);
    }

    pub fn load_module(&mut self, provider: &str, path: &str) -> Result<(), String> {
        if let Some(loader) = self.loaders.get(provider) {
            let exports = loader.load(path)?;
            for (name, func) in exports {
                self.functions.insert(name, func);
            }
            Ok(())
        } else {
            Err(format!("Unknown provider: {}", provider))
        }
    }

    pub fn register(&mut self, name: String, func: Box<dyn FFIFunction>) {
        self.functions.insert(name, func);
    }

    pub fn get(&self, name: &str) -> Option<&dyn FFIFunction> {
        self.functions.get(name).map(|f| f.as_ref())
    }
}
