use crate::{
    functions::FunctionType, helpers::IndexedIterator, ExternalSection, ExternalType, FunctionSection, GlobalSection, TypeItem,
    TypeSection, WasmSymbol, WasmVariable,
};
use indexmap::IndexMap;
use nyar_error::{FileSpan, NyarError};

mod interface;
mod wast_component;
mod wast_module;

#[derive(Default)]
pub struct ModuleBuilder {
    name: String,
    entry: String,
    memory_pages: u64,
    globals: GlobalSection,
    types: TypeSection,
    data: DataSection,
    functions: FunctionSection,
    externals: ExternalSection,
}

#[derive(Default)]
pub struct DataSection {
    data: IndexMap<String, DataItem>,
}

impl<'i> IntoIterator for &'i DataSection {
    type Item = (usize, &'i str, &'i DataItem);
    type IntoIter = IndexedIterator<'i, DataItem>;

    fn into_iter(self) -> Self::IntoIter {
        IndexedIterator::new(&self.data)
    }
}

pub struct DataItem {
    pub symbol: WasmSymbol,
    pub data: Vec<u8>,
    pub span: FileSpan,
}

impl DataSection {
    pub fn insert(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item.symbol.to_string(), item)
    }
}

impl DataItem {
    pub fn utf8(name: WasmSymbol, data: String) -> Self {
        Self { symbol: name, data: data.into_bytes(), span: FileSpan::default() }
    }
}

impl ModuleBuilder {
    pub fn new(memory: u64) -> Self {
        Self { memory_pages: memory, ..Default::default() }
    }

    pub fn get_module_name(&self) -> &str {
        &self.name
    }
    pub fn set_module_name<S: ToString>(&mut self, name: S) {
        self.name = name.to_string();
    }

    pub fn insert_type<T: Into<TypeItem>>(&mut self, t: T) -> Option<TypeItem> {
        self.types.insert(t.into())
    }
    pub fn insert_function(&mut self, f: FunctionType) {
        if f.entry {
            self.entry = f.name()
        }
        self.functions.insert(f)
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
