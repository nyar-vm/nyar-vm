use super::*;

mod display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiFlags {
    pub name: Arc<str>,
    pub variants: IndexMap<Arc<str>, WasiSemanticIndex>,
}

impl Display for WasiFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Hash for WasiFlags {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialOrd for WasiFlags {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for WasiFlags {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}
