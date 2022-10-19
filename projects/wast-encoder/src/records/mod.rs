use indexmap::IndexMap;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordType {
    pub symbol: Identifier,
    pub wasi_name: String,
    pub fields: IndexMap<Arc<str>, WasiParameter>,
}
