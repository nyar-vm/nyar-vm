use crate::vm::value::Value;
use nyar_types::NyarError;
use std::collections::HashMap;

pub type FFIResult = Result<Value, NyarError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FFIType {
    Null,
    Int,
    F32,
    F64,
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
    pub intrinsics: HashMap<u32, Box<dyn FFIFunction>>,
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
            intrinsics: HashMap::new(),
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

    pub fn register_intrinsic(&mut self, id: u32, func: Box<dyn FFIFunction>) {
        self.intrinsics.insert(id, func);
    }

    pub fn get(&self, name: &str) -> Option<&dyn FFIFunction> {
        self.functions.get(name).map(|f| f.as_ref())
    }

    pub fn get_intrinsic(&self, id: u32) -> Option<&dyn FFIFunction> {
        self.intrinsics.get(&id).map(|f| f.as_ref())
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
        
        // This is a simplified async yield simulation.
        // In a real VM, we might register a timer and yield.
        // For now, we just sleep synchronously to simulate work,
        // but we could also return VmError::YieldAsync if we had a timer system.
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(Value::null())
    }
}

pub struct NativePrint;
impl FFIFunction for NativePrint {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any],
            ret: FFIType::Null,
        })
    }
    fn call(&self, args: Vec<Value>) -> FFIResult {
        print!("{}", args[0]);
        Ok(Value::null())
    }
}

pub struct NativePrintln;
impl FFIFunction for NativePrintln {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any],
            ret: FFIType::Null,
        })
    }
    fn call(&self, args: Vec<Value>) -> FFIResult {
        println!("{}", args[0]);
        Ok(Value::null())
    }
}

pub struct NativeExit;
impl FFIFunction for NativeExit {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int],
            ret: FFIType::Null,
        })
    }
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let code = args[0].as_int() as i32;
        std::process::exit(code);
    }
}

pub struct NativeBitAnd;
impl FFIFunction for NativeBitAnd {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a & b))
    }
}

pub struct NativeBitOr;
impl FFIFunction for NativeBitOr {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a | b))
    }
}

pub struct NativeBitXor;
impl FFIFunction for NativeBitXor {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a ^ b))
    }
}

pub struct NativeBitNot;
impl FFIFunction for NativeBitNot {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        Ok(Value::int(!a))
    }
}

pub struct NativeBitShl;
impl FFIFunction for NativeBitShl {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a << b))
    }
}

pub struct NativeBitShr;
impl FFIFunction for NativeBitShr {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a >> b))
    }
}

pub struct NativePanic;
impl FFIFunction for NativePanic {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let msg = args.get(0).map(|v| v.to_string()).unwrap_or_else(|| "panic".to_string());
        panic!("{}", msg);
    }
}

pub struct NativeMathSin;
impl FFIFunction for NativeMathSin {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_float();
        Ok(Value::float(a.sin()))
    }
}

pub struct NativeMathCos;
impl FFIFunction for NativeMathCos {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_float();
        Ok(Value::float(a.cos()))
    }
}

pub struct NativeMathTan;
impl FFIFunction for NativeMathTan {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_float();
        Ok(Value::float(a.tan()))
    }
}

pub struct NativeMathSqrt;
impl FFIFunction for NativeMathSqrt {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_float();
        Ok(Value::float(a.sqrt()))
    }
}

pub struct NativeMathAbs;
impl FFIFunction for NativeMathAbs {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_float();
        Ok(Value::float(a.abs()))
    }
}

pub struct NativeMathRand;
impl FFIFunction for NativeMathRand {
    fn call(&self, _args: Vec<Value>) -> FFIResult {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Ok(Value::int(rng.gen::<i64>()))
    }
}

pub struct NativeMemAlloc;
impl FFIFunction for NativeMemAlloc {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let size = args[0].as_int() as usize;
        // In a real VM, this would allocate from a pool or GC heap
        // For now, we simulate with a raw allocation or similar
        let layout = std::alloc::Layout::from_size_align(size, 8).map_err(|_| NyarError::RuntimeError("Invalid layout".to_string()))?;
        unsafe {
            let ptr = std::alloc::alloc(layout);
            Ok(Value::int(ptr as i64))
        }
    }
}

pub struct NativeMemFree;
impl FFIFunction for NativeMemFree {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let ptr = args[0].as_int() as *mut u8;
        let size = args.get(1).map(|v| v.as_int() as usize).unwrap_or(0);
        if !ptr.is_null() && size > 0 {
            let layout = std::alloc::Layout::from_size_align(size, 8).map_err(|_| NyarError::RuntimeError("Invalid layout".to_string()))?;
            unsafe {
                std::alloc::dealloc(ptr, layout);
            }
        }
        Ok(Value::null())
    }
}

pub struct NativeMemRealloc;
impl FFIFunction for NativeMemRealloc {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let ptr = args[0].as_int() as *mut u8;
        let old_size = args[1].as_int() as usize;
        let new_size = args[2].as_int() as usize;
        let layout = std::alloc::Layout::from_size_align(old_size, 8).map_err(|_| NyarError::RuntimeError("Invalid layout".to_string()))?;
        unsafe {
            let new_ptr = std::alloc::realloc(ptr, layout, new_size);
            Ok(Value::int(new_ptr as i64))
        }
    }
}

pub struct NativeMemSet;
impl FFIFunction for NativeMemSet {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let ptr = args[0].as_int() as *mut u8;
        let val = args[1].as_int() as u8;
        let count = args[2].as_int() as usize;
        unsafe {
            std::ptr::write_bytes(ptr, val, count);
        }
        Ok(Value::null())
    }
}

pub struct NativeMemCopy;
impl FFIFunction for NativeMemCopy {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let dest = args[0].as_int() as *mut u8;
        let src = args[1].as_int() as *const u8;
        let count = args[2].as_int() as usize;
        unsafe {
            std::ptr::copy_nonoverlapping(src, dest, count);
        }
        Ok(Value::null())
    }
}

pub struct NativeStrLen;
impl FFIFunction for NativeStrLen {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let ptr = args[0].as_int() as *const i8;
        unsafe {
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            Ok(Value::int(len as i64))
        }
    }
}

pub struct NativeStrCmp;
impl FFIFunction for NativeStrCmp {
    fn call(&self, args: Vec<Value>) -> FFIResult {
        let s1 = args[0].as_int() as *const i8;
        let s2 = args[1].as_int() as *const i8;
        unsafe {
            let mut i = 0;
            while *s1.add(i) != 0 && *s1.add(i) == *s2.add(i) {
                i += 1;
            }
            Ok(Value::int((*s1.add(i) - *s2.add(i)) as i64))
        }
    }
}


