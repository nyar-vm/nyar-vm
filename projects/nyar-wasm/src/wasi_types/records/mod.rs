use super::*;
use crate::WasiValue;

mod arithmetic;
mod display;

#[derive(Debug, Clone)]
pub struct WasiRecordType {
    pub symbol: Identifier,
    pub wasi_name: String,
    pub fields: IndexMap<Arc<str>, WasiRecordField>,
}

impl Display for WasiRecordType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Hash)]
pub struct WasiRecordField {
    /// The name of the field
    pub name: Arc<str>,
    /// The WASI name of the field
    pub wasi_name: Arc<str>,
    /// The type of the parameter
    pub r#type: WasiType,
    /// The default value of the parameter
    pub default_value: Option<WasiValue>,
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
        Self { name: name.clone(), wasi_name: name.clone(), r#type: r#type.into(), default_value: None }
    }
}
