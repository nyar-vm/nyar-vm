use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
};

use indexmap::IndexMap;

mod display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiEnumeration {
    pub name: Arc<str>,
    pub variants: IndexMap<Arc<str>, WasiSemanticIndex>,
}

impl Display for WasiEnumeration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Hash for WasiEnumeration {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialOrd for WasiEnumeration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for WasiEnumeration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiSemanticIndex {
    pub name: Arc<str>,
    pub wasi_name: String,
}
