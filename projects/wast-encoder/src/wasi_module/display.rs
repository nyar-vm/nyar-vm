use super::*;

impl Debug for WasiInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasiInstance")
            .field("module", &self.module)
            .field("resources", &self.resources.values())
            .field("functions", &self.functions.values())
            .finish()
    }
}
