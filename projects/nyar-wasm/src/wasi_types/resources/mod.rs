use std::fmt::{Debug, Formatter, Write};

use crate::{
    dag::DependentGraph,
    helpers::{AliasExport, AliasOuter},
    DependenciesTrace, Identifier, WasiModule, WasiType, WastEncoder,
};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiResource {
    /// Resource language name
    pub symbol: Identifier,
    pub wasi_module: WasiModule,
    pub wasi_name: String,
}

impl Debug for WasiResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasiResource")
            .field("symbol", &self.symbol.to_string())
            .field("module", &self.wasi_module.to_string())
            .field("name", &self.wasi_name)
            .finish()
    }
}

impl WasiResource {
    pub(crate) fn write_wasi_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(export {} \"{}\" (type (sub resource)))", self.symbol.wasi_id(), self.wasi_name)
    }
}

impl AliasExport for WasiResource {
    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        let id = self.symbol.wasi_id();
        let name = self.wasi_name.as_str();
        write!(w, "(alias export ${module} \"{name}\" (type {id}))")
    }
}

impl AliasOuter for WasiResource {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        let root = &w.source.name;
        let id = self.symbol.wasi_id();
        write!(w, "(alias outer ${root} {id} (type {id}))")?;
        Ok(())
    }
}

impl WasiResource {
    pub fn new<S, M>(wasi_module: M, wasi_name: &str, name: S) -> Self
    where
        S: Into<Identifier>,
        M: Into<WasiModule>,
    {
        Self { symbol: name.into(), wasi_module: wasi_module.into(), wasi_name: wasi_name.to_string() }
    }
}

impl DependenciesTrace for WasiResource {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Resource(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, _: &'i DependentGraph, _: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
    }
}
