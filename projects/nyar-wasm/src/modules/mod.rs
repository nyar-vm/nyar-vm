use crate::{ExternalRegister, ExternalType, FunctionRegister, GlobalRegister, TypeItem, TypeRegister, WasmVariable};
use indexmap::IndexMap;
use nyar_error::{FileSpan, NyarError};
use nyar_hir::{FunctionType, IndexedIterator, Symbol};

mod interface;
mod wast_component;
mod wast_module;

#[derive(Default)]
pub struct ModuleBuilder {
    memory_pages: u64,
    globals: GlobalRegister,
    types: TypeRegister,
    data: DataBuilder,
    functions: FunctionRegister,
    externals: ExternalRegister,
}
#[derive(Default)]
pub struct DataBuilder {
    data: IndexMap<String, DataItem>,
}

impl<'i> IntoIterator for &'i DataBuilder {
    type Item = (usize, &'i str, &'i DataItem);
    type IntoIter = IndexedIterator<'i, DataItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.data)
    }
}

pub struct DataItem {
    pub symbol: Symbol,
    pub data: Vec<u8>,
    pub span: FileSpan,
}

impl DataBuilder {
    pub fn insert(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item.symbol.to_string(), item)
    }
}

impl DataItem {
    pub fn utf8(name: Symbol, data: String) -> Self {
        Self { symbol: name, data: data.into_bytes(), span: FileSpan::default() }
    }
}

impl ModuleBuilder {
    pub fn new(memory: u64) -> Self {
        Self { memory_pages: memory, ..Default::default() }
    }
    pub fn insert_type<T: Into<TypeItem>>(&mut self, t: T) -> Option<TypeItem> {
        self.types.insert(t.into())
    }
    pub fn insert_function(&mut self, f: FunctionType) {
        self.functions.add_native(f)
    }
    pub fn insert_external(&mut self, f: ExternalType) -> Option<ExternalType> {
        self.externals.insert(f)
    }
    pub fn insert_data(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item)
    }
    pub fn insert_global(&mut self, global: WasmVariable) -> Option<WasmVariable> {
        self.globals.insert(global)
    }
}
