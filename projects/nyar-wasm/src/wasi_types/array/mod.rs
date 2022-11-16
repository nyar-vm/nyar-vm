use super::*;

mod convert;
mod display;

/// A fixed-size array type in WASI
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiArrayType {
    /// The symbol of the array type
    pub symbol: Identifier,
    /// The inner type of the array
    pub r#type: WasiType,
    /// The inner mutable flag of the array
    pub mutable: bool,
    /// The length of the array
    pub length: Option<usize>,
}

impl WasiArrayType {
    /// Create a new array type
    pub fn new<T>(r#type: T) -> Self
    where
        T: Into<WasiType>,
    {
        Self { symbol: Identifier::new(""), r#type: r#type.into(), mutable: false, length: None }
    }
    /// Set the mutable flag of the array elements
    pub fn with_mutable(self, mutable: bool) -> Self {
        Self { mutable, ..self }
    }
}
