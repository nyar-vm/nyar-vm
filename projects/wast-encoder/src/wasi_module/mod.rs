use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
    ops::AddAssign,
    sync::Arc,
};

use crate::{ExternalFunction, Identifier, ResolveDependencies, WasiModule, WasiResource};

mod convert;
mod display;

pub struct WasiInstance {
    pub module: WasiModule,
    /// language_name: wasi_name
    pub resources: BTreeMap<Identifier, WasiResource>,
    pub functions: BTreeMap<Arc<str>, ExternalFunction>,
}

impl WasiInstance {
    pub fn new<M>(module: M) -> Self
    where
        M: Into<WasiModule>,
    {
        Self { module: module.into(), resources: Default::default(), functions: Default::default() }
    }
}
