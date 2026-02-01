use crate::vm::value::Value;
use crate::vm::VmError;
use std::collections::HashMap;

pub type FFIResult = Result<Value, VmError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FFIType {
    Null,
    Int,
    Float,
    Bool,
    String,
    List,
    Object,
    Any,
}

#[derive(Debug, Clone)]
pub struct FFISignature {
    pub params: Vec<FFIType>,
    pub ret: FFIType,
}

pub trait FFIFunction: Send + Sync {
    fn call(&self, args: Vec<Value>) -> FFIResult;
    fn signature(&self) -> Option<FFISignature> {
        None
    }
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

pub struct NativeAdd;
impl FFIFunction for NativeAdd {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int, FFIType::Int],
            ret: FFIType::Int,
        })
    }
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a + b))
    }
}

pub struct NativeGetTime;
impl FFIFunction for NativeGetTime {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![],
            ret: FFIType::Int,
        })
    }
    fn call(&self, _args: Vec<Value>) -> FFIResult {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(Value::int(now as i64))
    }
}

pub struct NativeSleep;
impl FFIFunction for NativeSleep {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int],
            ret: FFIType::Null,
        })
    }
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let ms = args[0].as_int() as u64;
        let start = std::time::Instant::now();
        
        // This is a simplified async yield simulation.
        // In a real VM, we might register a timer and yield.
        // For now, we just sleep synchronously to simulate work,
        // but we could also return VmError::YieldAsync if we had a timer system.
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(Value::null())
    }
}

