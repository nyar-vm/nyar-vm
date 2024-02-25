use crate::wasi_types::{AliasExport, AliasOuter};

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
            w.newline()?;
            wasi.write_wasi_define(w)?;
        }
        for imports in self.dependencies(&w.source.graph) {
            w.newline()?;
            imports.alias_outer(w)?;
        }
        for wasi in self.functions.values() {
            w.newline()?;
            w.export_function(wasi)?;
        }
        w.dedent(2);
        for wasi in self.resources.values() {
            w.newline()?;
            wasi.alias_export(w, &self.module)?;
        }
        for wasi in self.functions.values() {
            w.newline()?;
            wasi.alias_export(w, &self.module)?;
        }
        Ok(())
    }
}
