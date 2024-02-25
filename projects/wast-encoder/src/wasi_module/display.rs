use super::*;

impl Debug for WasiInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let resources: Vec<_> = self.resources.values().map(|s| s.wasi_name.as_str()).collect();
        let functions: Vec<_> = self.functions.values().map(|s| s.wasi_name.as_str()).collect();
        f.debug_struct("WasiInstance")
            .field("module", &self.module.to_string())
            .field("resources", &resources)
            .field("functions", &functions)
            .finish()
    }
}
