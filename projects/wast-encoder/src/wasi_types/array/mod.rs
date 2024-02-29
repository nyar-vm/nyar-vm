use super::*;

mod display;

///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiArrayType {
    symbol: Identifier,
    r#type: WasiType,
}

impl WasiArrayType {
    pub fn new<T>(r#type: T) -> Self
    where
        T: Into<WasiType>,
    {
        Self { symbol: Identifier::new(""), r#type: r#type.into() }
    }
}

impl From<WasiArrayType> for WasiType {
    fn from(value: WasiArrayType) -> Self {
        Self::Array(Box::new(value))
    }
}
