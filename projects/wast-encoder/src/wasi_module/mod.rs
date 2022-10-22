use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
    ops::AddAssign,
};

use crate::{ExternalFunction, Identifier, WasiModule, WasiResource, WasiType};

mod convert;
mod display;

pub struct WasiInstance {
    pub module: WasiModule,
    /// language_name: wasi_name
    pub resources: BTreeMap<Identifier, WasiResource>,
    pub functions: BTreeMap<Identifier, ExternalFunction>,
}

impl WasiInstance {
    pub fn new<M>(module: M) -> Self
    where
        M: Into<WasiModule>,
    {
        Self { module: module.into(), resources: Default::default(), functions: Default::default() }
    }
    pub fn insert(&mut self, wasi: &WasiType) {
        match wasi {
            WasiType::Integer8 { .. } => {}
            WasiType::Integer16 { .. } => {}
            WasiType::Integer32 { .. } => {}
            WasiType::Integer64 { .. } => {}
            WasiType::Option { .. } => {
                todo!()
            }
            WasiType::Result { .. } => {
                todo!()
            }
            WasiType::Resource(v) => {
                self.resources.insert(v.symbol.clone(), v.clone());
            }
            WasiType::Variant(_) => {
                todo!()
            }
            WasiType::TypeHandler { .. } => {
                todo!()
            }
            WasiType::TypeAlias { .. } => {
                todo!()
            }
            WasiType::External(v) => {
                self.functions.insert(v.symbol.clone(), *v.clone());
            }
        }
    }
}
