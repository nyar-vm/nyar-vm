use crate::vm::core::NyarVM;
use crate::vm::value::Value;
pub use nyar_types::QualifiedName;
use serde::{Deserialize, Serialize};

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

use std::sync::Arc;

impl NyarVM {
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

        self.builtins.insert(QualifiedName::new(vec!["System".to_string()]), system);
    }
}
