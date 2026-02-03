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
}

impl NyarVM {
    pub fn register_builtins(&mut self) {
        // Builtins are registered using their canonical names
        self.register_java_builtins();

        self.ffi.register_intrinsic(NyarBuiltin::NativeAdd as u32, Box::new(crate::vm::ffi::NativeAdd));
        self.ffi.register_intrinsic(NyarBuiltin::GetTime as u32, Box::new(crate::vm::ffi::NativeGetTime));
        self.ffi.register_intrinsic(NyarBuiltin::Sleep as u32, Box::new(crate::vm::ffi::NativeSleep));
        self.ffi.register_intrinsic(NyarBuiltin::Print as u32, Box::new(crate::vm::ffi::NativePrint));
        self.ffi.register_intrinsic(NyarBuiltin::Println as u32, Box::new(crate::vm::ffi::NativePrintln));
        self.ffi.register_intrinsic(NyarBuiltin::Exit as u32, Box::new(crate::vm::ffi::NativeExit));
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
