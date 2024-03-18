use super::*;
use crate::helpers::DependenciesTrace;

impl Hash for WasiEnumeration {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
    }
}

impl PartialOrd for WasiEnumeration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.symbol.partial_cmp(&other.symbol)
    }
}

impl Ord for WasiEnumeration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol.cmp(&other.symbol)
    }
}

impl From<WasiEnumeration> for WasiType {
    fn from(value: WasiEnumeration) -> Self {
        Self::Enums(value)
    }
}

impl DependenciesTrace for WasiEnumeration {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Enums(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, _: &'i DependentGraph, _: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
    }
}
