use super::*;

impl Hash for WasiFlags {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
    }
}

impl PartialOrd for WasiFlags {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.symbol.partial_cmp(&other.symbol)
    }
}

impl Ord for WasiFlags {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol.cmp(&other.symbol)
    }
}

impl From<WasiFlags> for WasiType {
    fn from(value: WasiFlags) -> Self {
        Self::Flags(value)
    }
}

impl DependenciesTrace for WasiFlags {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Flags(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, _: &'i DependentGraph, _: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
    }
}
