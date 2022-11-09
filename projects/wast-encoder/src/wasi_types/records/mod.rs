use super::*;

mod arithmetic;
mod display;

#[derive(Debug, Clone)]
pub struct WasiRecordType {
    pub symbol: Identifier,
    pub wasi_name: String,
    pub fields: IndexMap<Arc<str>, WasiRecordField>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiRecordField {
    pub name: Arc<str>,
    pub wasi_name: Arc<str>,
    /// The type of the parameter
    pub r#type: WasiType,
}

impl WasiRecordType {
    /// Create a new record type
    pub fn new(symbol: Identifier) -> Self {
        let wasi_name = symbol.wasi_name();
        Self { symbol, wasi_name, fields: IndexMap::new() }
    }
}
impl WasiRecordField {
    /// Create a new record type
    pub fn new<T>(name: Arc<str>, r#type: T) -> Self
    where
        T: Into<WasiType>,
    {
        Self { name: name.clone(), wasi_name: name.clone(), r#type: r#type.into() }
    }
}
