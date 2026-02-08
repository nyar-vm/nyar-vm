use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::effects::EffectHandler;
use nyar_types::{EffectInfo, NyarError};
pub use nyar_types::QualifiedName;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod platform;

/// Nyar 标准内建函数定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum NyarBuiltin {
    /// Print to standard output
    Print = 1,
    /// Print to standard output with a newline
    Println = 2,
    /// Exit the process with a status code
    Exit = 3,
    /// Get the current system time
    GetTime = 4,
    /// Sleep for a duration in milliseconds
    Sleep = 5,
    /// Add two integers (native implementation)
    NativeAdd = 6,
    /// Panic with a message
    Panic = 7,
    /// Sine function
    MathSin = 8,
    /// Square root function
    MathSqrt = 9,
    /// Memory allocation
    MemAlloc = 10,
    /// Absolute value
    MathAbs = 11,
    /// Cosine function
    MathCos = 12,
    /// Tangent function
    MathTan = 13,
    /// Bitwise AND
    BitAnd = 14,
    /// Bitwise OR
    BitOr = 15,
    /// Bitwise XOR
    BitXor = 16,
    /// Bitwise NOT
    BitNot = 17,
    /// Bitwise Left Shift
    BitShl = 18,
    /// Bitwise Right Shift
    BitShr = 19,
    /// Memory free
    MemFree = 20,
    /// Memory realloc
    MemRealloc = 21,
    /// Memory set
    MemSet = 22,
    /// Memory copy
    MemCopy = 23,
    /// String length
    StrLen = 24,
    /// String compare
    StrCmp = 25,
    /// Random number
    MathRand = 26,
}

pub struct DefaultEffectHandler;

impl EffectHandler for DefaultEffectHandler {
    fn perform_effect(
        &self,
        vm: &mut NyarVM,
        _module_idx: usize,
        effect: EffectInfo,
        args: Vec<Value>,
    ) -> Result<Option<Value>, NyarError> {
        if effect.name.parts.len() == 1 {
            let name = &effect.name.parts[0];
            match name.as_str() {
                "LoggerEvent" | "print" => {
                    for arg in &args {
                        vm.platform.stdout_write(&format!("{}", arg));
                    }
                    return Ok(None);
                }
                "exit" => {
                    let code = args.get(0).map(|v| v.as_int()).unwrap_or(0) as i32;
                    vm.platform.proc_exit(code);
                }
                "now" => {
                    let now = vm.platform.clock_now();
                    return Ok(Some(Value::float(now)));
                }
                "get_env" => {
                    if let Some(key) = args.get(0).and_then(|v| v.try_as_str()) {
                        if let Some(val) = vm.platform.get_env(key) {
                            return Ok(Some(Value::string(val, &vm.gc)));
                        }
                    }
                    return Ok(Some(Value::null()));
                }
                "add" => {
                    let a = args.get(0).cloned().unwrap_or(Value::null());
                    let b = args.get(1).cloned().unwrap_or(Value::null());
                    let res = match (a.tag(), b.tag()) {
                        (crate::vm::value::ValueTag::Int, crate::vm::value::ValueTag::Int) => {
                            Value::int(a.as_int() + b.as_int())
                        }
                        (crate::vm::value::ValueTag::F32, crate::vm::value::ValueTag::F32) => {
                            Value::f32(a.as_f32() + b.as_f32())
                        }
                        (crate::vm::value::ValueTag::F64, crate::vm::value::ValueTag::F64) => {
                            Value::float(a.as_f64() + b.as_f64())
                        }
                        (crate::vm::value::ValueTag::String, crate::vm::value::ValueTag::String) => {
                            let mut s = a.try_as_str().unwrap_or("").to_string();
                            s.push_str(b.try_as_str().unwrap_or(""));
                            Value::string(s, &vm.gc)
                        }
                        _ => Value::null(),
                    };
                    return Ok(Some(res));
                }
                "Fetch" => {
                    if let Some(func) = vm.ffi.get("std.http.get") {
                        let res = func.call(vm, args).map_err(|e| vm.error(nyar_types::VmErrorKind::RuntimeError(e.to_string())))?;
                        return Ok(Some(res));
                    }
                }
                "delay" => {
                    if let Some(func) = vm.ffi.get("std.async.delay") {
                        let res = func.call(vm, args).map_err(|e| vm.error(nyar_types::VmErrorKind::RuntimeError(e.to_string())))?;
                        return Ok(Some(res));
                    }
                }
                "spawn" => {
                    if let Some(func) = vm.ffi.get("std.async.spawn") {
                        let res = func.call(vm, args).map_err(|e| vm.error(nyar_types::VmErrorKind::RuntimeError(e.to_string())))?;
                        return Ok(Some(res));
                    }
                }
                _ => {}
            }
        }

        // Fallback to FFI for std.* effects if not handled above
        let effect_name = effect.name.to_string();
        if effect_name.starts_with("std.") {
            if let Some(func) = vm.ffi.get(&effect_name) {
                let res = func.call(vm, args).map_err(|e| vm.error(nyar_types::VmErrorKind::RuntimeError(e.to_string())))?;
                return Ok(Some(res));
            }
        }

        Err(vm.error(nyar_types::VmErrorKind::UnhandledEffect(effect.name)))
    }
}

impl NyarVM {
    pub fn setup_default_runtime(&mut self) {
        #[cfg(not(target_os = "unknown"))]
        {
            self.platform = Arc::new(platform::NativePlatform);
        }
        #[cfg(target_os = "unknown")]
        {
            #[cfg(feature = "wasi")]
            {
                self.platform = Arc::new(platform::WasiPlatform);
            }
            #[cfg(not(feature = "wasi"))]
            {
                self.platform = Arc::new(crate::vm::platform::StubPlatform);
            }
        }
        
        self.ffi.register_std();
        self.register_builtins();
        self.effect_handler = Some(Arc::new(DefaultEffectHandler));
    }

    pub fn register_builtins(&mut self) {
        // Builtins are registered using their canonical names
        self.register_java_builtins();

        self.ffi.register_intrinsic(NyarBuiltin::Print as u32, Arc::new(crate::vm::ffi::NativePrint));
        self.ffi.register_intrinsic(NyarBuiltin::Println as u32, Arc::new(crate::vm::ffi::NativePrintln));
        self.ffi.register("nyar:std::io:println".to_string(), Arc::new(crate::vm::ffi::NativePrintln));
        self.ffi.register("nyar:std::io:print".to_string(), Arc::new(crate::vm::ffi::NativePrint));
        self.ffi.register("println".to_string(), Arc::new(crate::vm::ffi::NativePrintln));
        self.ffi.register("print".to_string(), Arc::new(crate::vm::ffi::NativePrint));
        self.ffi.register_intrinsic(NyarBuiltin::Exit as u32, Arc::new(crate::vm::ffi::NativeExit));
        self.ffi.register_intrinsic(NyarBuiltin::GetTime as u32, Arc::new(crate::vm::ffi::NativeGetTime));
        self.ffi.register_intrinsic(NyarBuiltin::Sleep as u32, Arc::new(crate::vm::ffi::NativeSleep));
        self.ffi.register_intrinsic(NyarBuiltin::NativeAdd as u32, Arc::new(crate::vm::ffi::NativeAdd));
        
        self.ffi.register_intrinsic(NyarBuiltin::BitAnd as u32, Arc::new(crate::vm::ffi::NativeBitAnd));
        self.ffi.register_intrinsic(NyarBuiltin::BitOr as u32, Arc::new(crate::vm::ffi::NativeBitOr));
        self.ffi.register_intrinsic(NyarBuiltin::BitXor as u32, Arc::new(crate::vm::ffi::NativeBitXor));
        self.ffi.register_intrinsic(NyarBuiltin::BitNot as u32, Arc::new(crate::vm::ffi::NativeBitNot));
        self.ffi.register_intrinsic(NyarBuiltin::BitShl as u32, Arc::new(crate::vm::ffi::NativeBitShl));
        self.ffi.register_intrinsic(NyarBuiltin::BitShr as u32, Arc::new(crate::vm::ffi::NativeBitShr));
        self.ffi.register_intrinsic(NyarBuiltin::Panic as u32, Arc::new(crate::vm::ffi::NativePanic));
        self.ffi.register_intrinsic(NyarBuiltin::MathSin as u32, Arc::new(crate::vm::ffi::NativeMathSin));
        self.ffi.register_intrinsic(NyarBuiltin::MathCos as u32, Arc::new(crate::vm::ffi::NativeMathCos));
        self.ffi.register_intrinsic(NyarBuiltin::MathTan as u32, Arc::new(crate::vm::ffi::NativeMathTan));
        self.ffi.register_intrinsic(NyarBuiltin::MathSqrt as u32, Arc::new(crate::vm::ffi::NativeMathSqrt));
        self.ffi.register_intrinsic(NyarBuiltin::MathAbs as u32, Arc::new(crate::vm::ffi::NativeMathAbs));
        self.ffi.register_intrinsic(NyarBuiltin::MathRand as u32, Arc::new(crate::vm::ffi::NativeMathRand));
        self.ffi.register_intrinsic(NyarBuiltin::MemAlloc as u32, Arc::new(crate::vm::ffi::NativeMemAlloc));
        self.ffi.register_intrinsic(NyarBuiltin::MemFree as u32, Arc::new(crate::vm::ffi::NativeMemFree));
        self.ffi.register_intrinsic(NyarBuiltin::MemRealloc as u32, Arc::new(crate::vm::ffi::NativeMemRealloc));
        self.ffi.register_intrinsic(NyarBuiltin::MemSet as u32, Arc::new(crate::vm::ffi::NativeMemSet));
        self.ffi.register_intrinsic(NyarBuiltin::MemCopy as u32, Arc::new(crate::vm::ffi::NativeMemCopy));
        self.ffi.register_intrinsic(NyarBuiltin::StrLen as u32, Arc::new(crate::vm::ffi::NativeStrLen));
        self.ffi.register_intrinsic(NyarBuiltin::StrCmp as u32, Arc::new(crate::vm::ffi::NativeStrCmp));
    }

    fn register_java_builtins(&mut self) {
        let out = Value::dyn_object(&self.gc);

        // System.out.println -> standard mapping
        let system = Value::dyn_object(&self.gc);
        if let Some(system_mut) = system.try_as_dyn_object_mut() {
            system_mut.entries.insert("out".to_string(), out);
        }

        self.env.builtins.insert(QualifiedName::new(vec!["System".to_string()]), system);
    }
}
