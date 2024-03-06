use super::*;

mod display;

///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiArrayType {
    symbol: Identifier,
    r#type: WasiType,
    mutable: bool,
}

impl Debug for WasiArrayType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl WasiArrayType {
    /// Create a new array type
    pub fn new<T>(r#type: T) -> Self
    where
        T: Into<WasiType>,
    {
        Self { symbol: Identifier::new(""), r#type: r#type.into(), mutable: false }
    }
    /// Set the mutable flag of the array elements
    pub fn with_mutable(self, mutable: bool) -> Self {
        Self { mutable, ..self }
    }
}

impl From<WasiArrayType> for WasiType {
    fn from(value: WasiArrayType) -> Self {
        Self::Array(Box::new(value))
    }
}
