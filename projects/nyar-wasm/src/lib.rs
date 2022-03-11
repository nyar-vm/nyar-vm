use nyar_error::NyarError;
use wasmtime::*;

pub struct StartupConfig {}

pub struct WasmEngine {
    store: Store<StartupConfig>,
}

impl WasmEngine {
    pub fn new(start: StartupConfig) -> Self {
        let engine = Engine::default();
        let store = Store::new(&engine, start);
        Self { store }
    }
    /// Evaluate a wat file
    pub fn evaluate_wat(&mut self, script: &str) -> Result<(), NyarError> {
        let engine = self.store.engine();
        let module = Module::new(engine, script)?;
        let instance = Instance::new(&mut self.store, &module, &[])?;
        let entry = match instance.get_func(&mut self.store, "_start") {
            Some(func) => func,
            None => Err(NyarError::runtime_error("Entry function `_start` not found"))?,
        };
        let mut out = vec![Val::I32(0)];
        entry.call(&mut self.store, &[], &mut out)?;
        println!("Result: {:?}", out);
        Ok(())
    }
}
