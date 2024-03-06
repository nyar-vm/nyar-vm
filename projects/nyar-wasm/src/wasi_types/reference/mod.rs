use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiTypeReference {
    pub symbol: Identifier,
    pub owner: WasiOwnership,
}

impl Display for WasiTypeReference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<WasiTypeReference> for WasiType {
    fn from(value: WasiTypeReference) -> Self {
        Self::TypeHandler(value)
    }
}

impl WasiTypeReference {
    pub fn new(symbol: Identifier) -> Self {
        Self { symbol, owner: WasiOwnership::Normal }
    }
    pub fn owned(symbol: Identifier) -> Self {
        Self { symbol, owner: WasiOwnership::Owned }
    }
    pub fn borrow(symbol: Identifier) -> Self {
        Self { symbol, owner: WasiOwnership::Borrow }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiOwnership {
    Normal,
    Owned,
    Borrow,
}

impl TypeReference for WasiTypeReference {
    fn upper_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self.owner {
            WasiOwnership::Normal => {
                write!(w, "{}", self.symbol.wasi_id())
            }
            WasiOwnership::Owned => {
                write!(w, "(own {})", self.symbol.wasi_id())
            }
            WasiOwnership::Borrow => {
                write!(w, "(borrow {})", self.symbol.wasi_id())
            }
        }
    }

    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.source.graph.get(self).lower_type(w)
    }

    fn lower_type_inner<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.source.graph.get(self).lower_type_inner(w)
    }
}
