use crate::vm::core::NyarVM;
use crate::vm::value::Value;

impl NyarVM {
    pub fn register_builtins(&mut self) {
        // Builtins can be registered here
        self.register_java_builtins();
    }

    fn register_java_builtins(&mut self) {
        let out = Value::dyn_object(&self.gc);

        // System.out.println
        // For now, System.out is just a DynObject

        let system = Value::dyn_object(&self.gc);
        if let Some(system_mut) = system.try_as_dyn_object_mut() {
            system_mut.entries.insert("out".to_string(), out);
        }

        self.builtins.insert("System".to_string(), system);
    }
}
