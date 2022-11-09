use super::*;
use crate::WasiVariantItem;
use std::{cmp::Ordering, ops::AddAssign};

#[derive(Debug, Clone)]
pub struct WasiRecordType {
    pub symbol: Identifier,
    pub wasi_name: String,
    pub fields: IndexMap<Arc<str>, WasiRecordField>,
}

impl Eq for WasiRecordType {}

impl PartialEq for WasiRecordType {
    fn eq(&self, other: &Self) -> bool {
        self.symbol.eq(&other.symbol)
    }
}

impl PartialOrd for WasiRecordType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.symbol.partial_cmp(&other.symbol)
    }
}

impl Ord for WasiRecordType {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol.cmp(&other.symbol)
    }
}

impl Hash for WasiRecordType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state)
    }
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

impl DependenciesTrace for WasiRecordType {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Record(self.clone()));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.fields.iter().for_each(|(_, v)| v.collect_wasi_types(dict, collected));
    }
}

impl DependenciesTrace for WasiRecordField {
    fn define_language_types(&self, _: &mut DependentGraph) {
        unreachable!()
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.r#type.collect_wasi_types(dict, collected)
    }
}

impl AddAssign<WasiRecordField> for WasiRecordType {
    fn add_assign(&mut self, rhs: WasiRecordField) {
        self.fields.insert(rhs.name.clone(), rhs);
    }
}
