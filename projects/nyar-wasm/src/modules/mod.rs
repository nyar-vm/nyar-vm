use crate::{
    functions::FunctionType,
    helpers::{write_wasm_bytes, IntoWasm, WasmName},
    DataItem, DataSection, ExternalSection, ExternalType, GlobalSection, TypeSection, WasmType, WasmVariable,
};
use nyar_error::NyarError;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
use wast::{
    component::{
        Component, ComponentDefinedType, ComponentField, ComponentKind, ComponentValType, CoreModule, CoreModuleKind,
        PrimitiveValType, Type, TypeDef,
    },
    core::{InlineExport, Limits, Memory, MemoryKind, MemoryType, Module, ModuleField, ModuleKind, Producers},
    token::{Index, NameAnnotation, Span},
};
mod wast_component;
mod wast_module;

#[derive(Default)]
pub struct WasmBuilder {
    pub name: String,
    pub entry: String,
    pub memory_pages: u64,
    pub globals: GlobalSection,
    pub types: TypeSection,
    pub data: DataSection,
    pub functions: BTreeMap<String, FunctionType>,
    pub externals: ExternalSection,
}

impl WasmBuilder {
    pub fn new<S: ToString>(name: S) -> Self {
        Self { name: name.to_string(), ..Default::default() }
    }

    pub fn get_module_name(&self) -> &str {
        &self.name
    }
    pub fn set_module_name<S: ToString>(&mut self, name: S) {
        self.name = name.to_string();
    }

    pub fn insert_type<T: Into<WasmType>>(&mut self, t: T) -> Option<WasmType> {
        self.types.insert(t.into())
    }
    pub fn insert_function(&mut self, f: FunctionType) {
        if f.entry {
            self.entry = f.name()
        }
        self.functions.insert(f.name(), f);
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
