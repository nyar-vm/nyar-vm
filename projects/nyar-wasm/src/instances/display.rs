use super::*;

impl Debug for WasiInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let resources: Vec<_> = self.resources.values().map(|s| s.wasi_name.to_string()).collect();
        let functions: Vec<_> = self.functions.values().map(|s| &s.symbol).collect();

        f.debug_struct("WasiInstance")
            .field("module", &self.module.to_string())
            .field("resources", &resources)
            .field("functions", &functions)
            .finish()
    }
}

impl ComponentDefine for WasiInstance {
    fn wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(import \"{name}\" (instance ${name}", name = self.module)?;
        w.indent();
        for wasi in self.resources.values() {
            w.newline()?;
            wasi.write_wasi_define(w)?;
        }
        self.alias_outer(w)?;
        w.dedent(2);
        self.alias_export(w, &self.module)
    }

    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        for imports in self.dependencies(&w.source.graph) {
            imports.alias_outer(w)?;
        }
        for wasi in self.functions.values() {
            w.newline()?;
            wasi.alias_outer(w)?;
        }
        Ok(())
    }

    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        for wasi in self.resources.values() {
            w.newline()?;
            wasi.alias_export(w, module)?;
        }
        for wasi in self.functions.values() {
            w.newline()?;
            wasi.alias_export(w, module)?;
        }
        Ok(())
    }
}
