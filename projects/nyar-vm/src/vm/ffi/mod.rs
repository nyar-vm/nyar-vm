pub mod io;
pub mod fs;
pub mod http;
pub mod json;
pub mod net;
pub mod async_ffi;

use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;
use dashmap::DashMap;
use std::sync::Arc;

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
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult;
    fn signature(&self) -> Option<FFISignature> {
        None
    }
}

#[derive(Clone)]
pub struct FFIRegistry {
    pub functions: Arc<DashMap<String, Arc<dyn FFIFunction>>>,
    pub intrinsics: Arc<DashMap<u32, Arc<dyn FFIFunction>>>,
    pub loaders: Arc<DashMap<String, Arc<dyn ModuleLoader>>>,
}

pub trait ModuleLoader: Send + Sync {
    fn load(&self, path: &str) -> Result<Vec<(String, Arc<dyn FFIFunction>)>, NyarError>;
}

impl Default for FFIRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FFIRegistry {
    pub fn new() -> Self {
        Self {
            functions: Arc::new(DashMap::new()),
            intrinsics: Arc::new(DashMap::new()),
            loaders: Arc::new(DashMap::new()),
        }
    }

    pub fn register_loader(&mut self, name: impl Into<String>, loader: Arc<dyn ModuleLoader>) {
        self.loaders.insert(name.into(), loader);
    }

    pub fn load_module(&mut self, provider: &str, path: &str) -> Result<(), String> {
        if let Some(loader) = self.loaders.get(provider) {
            let exports = loader.load(path).map_err(|e| e.to_string())?;
            for (name, func) in exports {
                self.functions.insert(name, func);
            }
            Ok(())
        } else {
            Err(format!("Unknown provider: {}", provider))
        }
    }

    pub fn register(&mut self, name: String, func: Arc<dyn FFIFunction>) {
        self.functions.insert(name, func);
    }

    pub fn register_intrinsic(&mut self, id: u32, func: Arc<dyn FFIFunction>) {
        self.intrinsics.insert(id, func);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn FFIFunction>> {
        self.functions.get(name).map(|r| r.value().clone())
    }

    pub fn get_intrinsic(&self, id: u32) -> Option<Arc<dyn FFIFunction>> {
        self.intrinsics.get(&id).map(|r| r.value().clone())
    }

    pub fn register_std(&mut self) {
        self.register("std.io.print".to_string(), Arc::new(io::StdIoPrint));
        self.register("std.io.println".to_string(), Arc::new(io::StdIoPrintln));
        self.register("std.io.read_line".to_string(), Arc::new(io::StdIoReadLine));
        
        self.register("std.fs.read_to_string".to_string(), Arc::new(fs::StdFsReadToString));
        self.register("std.fs.write".to_string(), Arc::new(fs::StdFsWrite));
        self.register("std.fs.exists".to_string(), Arc::new(fs::StdFsExists));
        self.register("std.fs.remove_file".to_string(), Arc::new(fs::StdFsRemoveFile));

        self.register("std.http.get".to_string(), Arc::new(http::StdHttpGet));
        self.register("std.http.post".to_string(), Arc::new(http::StdHttpPost));
        self.register("std.http.set_proxy".to_string(), Arc::new(http::StdHttpSetProxy));

        self.register("std.config.json.Json.parse".to_string(), Arc::new(json::StdJsonParse));
        self.register("std.config.json.Json.stringify".to_string(), Arc::new(json::StdJsonStringify));

        self.register("std.net.TcpStream.connect".to_string(), Arc::new(net::TcpConnect));
        self.register("std.net.TcpStream.read".to_string(), Arc::new(net::TcpRead));
        self.register("std.net.TcpStream.write".to_string(), Arc::new(net::TcpWrite));
        self.register("std.net.TcpStream.close".to_string(), Arc::new(net::TcpClose));
        self.register("std.net.TcpListener.listen".to_string(), Arc::new(net::TcpListen));
        self.register("std.net.TcpListener.accept".to_string(), Arc::new(net::TcpAccept));
        self.register("std.net.TcpListener.close".to_string(), Arc::new(net::TcpClose));

        self.register("std.async.Async.delay".to_string(), Arc::new(async_ffi::AsyncDelay));
        self.register("std.async.Async.spawn".to_string(), Arc::new(async_ffi::AsyncSpawn));
        self.register("std.async.Async.await".to_string(), Arc::new(async_ffi::AsyncAwait));
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
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
    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
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
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        if !args.is_empty() {
            vm.log(&format!("{}", args[0]));
        }
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
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        if !args.is_empty() {
            vm.log(&format!("{}", args[0]));
        }
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let code = args[0].as_int() as i32;
        std::process::exit(code);
    }
}

pub struct NativeBitAnd;
impl FFIFunction for NativeBitAnd {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a & b))
    }
}

pub struct NativeBitOr;
impl FFIFunction for NativeBitOr {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a | b))
    }
}

pub struct NativeBitXor;
impl FFIFunction for NativeBitXor {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a ^ b))
    }
}

pub struct NativeBitNot;
impl FFIFunction for NativeBitNot {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        Ok(Value::int(!a))
    }
}

pub struct NativeBitShl;
impl FFIFunction for NativeBitShl {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a << b))
    }
}

pub struct NativeBitShr;
impl FFIFunction for NativeBitShr {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].as_int();
        let b = args[1].as_int();
        Ok(Value::int(a >> b))
    }
}

pub struct NativePanic;
impl FFIFunction for NativePanic {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let msg = args.get(0).map(|v| v.to_string()).unwrap_or_else(|| "panic".to_string());
        panic!("{}", msg);
    }
}

pub struct NativeMathSin;
impl FFIFunction for NativeMathSin {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].to_f64();
        Ok(Value::float(a.sin()))
    }
}

pub struct NativeMathCos;
impl FFIFunction for NativeMathCos {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].to_f64();
        Ok(Value::float(a.cos()))
    }
}

pub struct NativeMathTan;
impl FFIFunction for NativeMathTan {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].to_f64();
        Ok(Value::float(a.tan()))
    }
}

pub struct NativeMathSqrt;
impl FFIFunction for NativeMathSqrt {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].to_f64();
        Ok(Value::float(a.sqrt()))
    }
}

pub struct NativeMathAbs;
impl FFIFunction for NativeMathAbs {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let a = args[0].to_f64();
        Ok(Value::float(a.abs()))
    }
}

pub struct NativeMathRand;
impl FFIFunction for NativeMathRand {
    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Ok(Value::int(rng.gen::<i64>()))
    }
}

pub struct NativeMemAlloc;
impl FFIFunction for NativeMemAlloc {
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let size = args[0].as_int() as usize;
        Ok(Value::bytes(vec![0u8; size], &vm.gc))
    }
}

pub struct NativeMemFree;
impl FFIFunction for NativeMemFree {
    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        // Nyar uses GC for memory management, explicit free is a no-op for managed objects.
        Ok(Value::null())
    }
}

pub struct NativeMemRealloc;
impl FFIFunction for NativeMemRealloc {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let val = args[0];
        let new_size = args[2].as_int() as usize;
        if let Some(bytes) = val.try_as_bytes_mut() {
            bytes.data.resize(new_size, 0);
            Ok(val)
        } else {
            // Fallback for raw pointers (deprecated)
            let ptr = args[0].as_int() as *mut u8;
            let old_size = args[1].as_int() as usize;
            let layout = std::alloc::Layout::from_size_align(old_size, 8)
                .map_err(|_| NyarError::RuntimeError("Invalid layout".to_string()))?;
            unsafe {
                let new_ptr = std::alloc::realloc(ptr, layout, new_size);
                Ok(Value::int(new_ptr as i64))
            }
        }
    }
}

pub struct NativeMemSet;
impl FFIFunction for NativeMemSet {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let ptr = args[0].as_raw_ptr();
        let val = args[1].as_int() as u8;
        let count = args[2].as_int() as usize;
        if !ptr.is_null() {
            unsafe {
                std::ptr::write_bytes(ptr, val, count);
            }
        }
        Ok(Value::null())
    }
}

pub struct NativeMemCopy;
impl FFIFunction for NativeMemCopy {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let dest = args[0].as_raw_ptr();
        let src = args[1].as_raw_ptr();
        let count = args[2].as_int() as usize;
        if !dest.is_null() && !src.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(src, dest, count);
            }
        }
        Ok(Value::null())
    }
}

pub struct NativeStrLen;
impl FFIFunction for NativeStrLen {
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        if let Some(s) = args[0].try_as_str() {
            return Ok(Value::int(s.len() as i64));
        }
        if let Some(b) = args[0].try_as_bytes() {
            return Ok(Value::int(b.data.len() as i64));
        }
        let ptr = args[0].as_raw_ptr() as *const i8;
        if ptr.is_null() {
            return Ok(Value::int(0));
        }
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let s1 = args[0].as_raw_ptr() as *const i8;
        let s2 = args[1].as_raw_ptr() as *const i8;
        if s1.is_null() || s2.is_null() {
            return Ok(Value::int(if s1 == s2 { 0 } else { 1 }));
        }
        unsafe {
            let mut i = 0;
            while *s1.add(i) != 0 && *s1.add(i) == *s2.add(i) {
                i += 1;
            }
            Ok(Value::int((*s1.add(i) - *s2.add(i)) as i64))
        }
    }
}


