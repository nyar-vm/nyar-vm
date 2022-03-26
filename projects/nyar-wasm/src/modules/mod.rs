use crate::helpers::WasmEmitter;
use indexmap::IndexMap;
use nyar_error::NyarError;
use nyar_hir::{ExternalType, FunctionItem, FunctionRegister, GlobalBuilder, Identifier, NamedValue, TypeBuilder, TypeItem};

use crate::{functions::WasmFunctionBody, values::WasmVariable};
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

pub struct DataItem {
    name: Identifier,
    data: Vec<u8>,
}

impl DataBuilder {
    pub fn insert(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item.name.to_string(), item)
    }
}

impl DataItem {
    pub fn utf8(name: Identifier, data: String) -> Self {
        Self { name, data: data.into_bytes() }
    }
}

impl ModuleBuilder {
    pub fn new(memory: u64) -> Self {
        Self { memory_pages: memory, ..Default::default() }
    }
    pub fn insert_type<T: Into<TypeItem>>(&mut self, t: T) -> Option<TypeItem> {
        self.types.insert(t.into())
    }
    pub fn insert_function(&mut self, f: FunctionItem) {
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
