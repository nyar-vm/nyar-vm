use crate::wasi_types::AliasExport;

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

impl ComponentDefine for WasiInstance {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(import \"{name}\" (instance ${name}", name = self.module)?;
        w.indent();
        for wasi in self.resources.values() {
            wasi.write_wasi_define(w)?;
            w.newline()?;
        }
        for wasi in self.functions.values() {
            w.export_function(wasi)?;
            w.newline()?
        }
        w.dedent(2);
        w.newline()?;
        for wasi in self.resources.values() {
            wasi.alias_export(w, &self.module)?;
            w.newline()?
        }
        for wasi in self.functions.values() {
            wasi.alias_export(w, &self.module)?;
            w.newline()?
        }
        Ok(())
    }
}
