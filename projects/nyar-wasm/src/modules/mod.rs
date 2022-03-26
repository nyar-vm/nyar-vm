use crate::{functions::WasmFunctionBody, helpers::WasmEmitter};
use indexmap::IndexMap;
use nyar_error::{FileSpan, NyarError};
use nyar_hir::{
    ExternalType, FunctionRegister, FunctionType, GlobalBuilder, Identifier, IndexedIterator, NamedValue, Symbol, TypeBuilder,
    TypeItem,
};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, ElementSection, Elements, ExportKind, ExportSection, GlobalSection, MemorySection,
    MemoryType, RefType, StartSection, TableSection, TableType,
};
mod interface;
mod wast_component;
mod wast_module;

#[derive(Default)]
pub struct ModuleBuilder {
    memory_pages: u64,
    globals: GlobalBuilder,
    types: TypeBuilder,
    data: DataBuilder,
    functions: FunctionRegister,
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
    pub fn insert_external(&mut self, f: ExternalType) {
        self.functions.add_external(f)
    }

    pub fn insert_data(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item)
    }

    pub fn insert_global(&mut self, global: NamedValue) -> Option<NamedValue> {
        self.globals.insert(global)
    }
}
