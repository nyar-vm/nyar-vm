use crate::vm::core::NyarVM;
use crate::vm::value::Value;
pub use nyar_types::{NyarBuiltin, QualifiedName};
use serde::{Deserialize, Serialize};

impl NyarVM {
    pub fn register_builtins(&mut self) {
        // Builtins are registered using their canonical names
        self.register_java_builtins();
        
        self.ffi.register(NyarBuiltin::NativeAdd.path().to_string(), Box::new(crate::vm::ffi::NativeAdd));
        self.ffi.register(NyarBuiltin::GetTime.path().to_string(), Box::new(crate::vm::ffi::NativeGetTime));
        self.ffi.register(NyarBuiltin::Sleep.path().to_string(), Box::new(crate::vm::ffi::NativeSleep));
        self.ffi.register(NyarBuiltin::Print.path().to_string(), Box::new(crate::vm::ffi::NativePrint));
        self.ffi.register(NyarBuiltin::Println.path().to_string(), Box::new(crate::vm::ffi::NativePrintln));
        self.ffi.register(NyarBuiltin::Exit.path().to_string(), Box::new(crate::vm::ffi::NativeExit));
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
