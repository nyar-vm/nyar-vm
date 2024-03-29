use crate::{
    functions::FunctionType,
    helpers::{write_wasm_bytes, IntoWasm, WasmName},
    structures::StructureType,
    ArrayType, DataItem, ExternalFunctionType, WasmExternalName, WasmVariable,
};
use nyar_error::NyarError;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use wast::{
    component::{Component, ComponentField, ComponentKind, CoreModule, CoreModuleKind},
    core::{InlineExport, Limits, Memory, MemoryKind, MemoryType, Module, ModuleField, ModuleKind, Producers},
    token::{NameAnnotation, Span},
};

mod wast_component;
mod wast_module;

pub trait WasmItem {
    fn register(self, builder: &mut WasmBuilder);
}

#[derive(Debug, Default)]
pub struct WasmBuilder {
    pub name: String,
    pub entry: String,
    pub memory_pages: u64,
    pub globals: BTreeMap<String, WasmVariable>,
    pub structures: BTreeMap<String, StructureType>,
    pub arrays: BTreeMap<String, ArrayType>,
    pub data: BTreeMap<String, DataItem>,
    pub functions: BTreeMap<String, FunctionType>,
    pub externals: BTreeMap<String, ExternalFunctionType>,
}

impl WasmItem for ExternalFunctionType {
    fn register(self, builder: &mut WasmBuilder) {
        builder.externals.insert(self.name().to_string(), self);
    }
}

impl WasmItem for FunctionType {
    fn register(self, builder: &mut WasmBuilder) {
        if self.entry {
            builder.entry = self.name()
        }
        builder.functions.insert(self.name(), self);
    }
}

impl WasmItem for ArrayType {
    fn register(self, builder: &mut WasmBuilder) {
        builder.arrays.insert(self.symbol.to_string(), self);
    }
}

impl WasmBuilder {
    pub fn new<S: ToString>(name: S) -> Self {
        Self { name: name.to_string(), ..Default::default() }
    }

    pub fn get_instance_name(&self) -> &str {
        &self.name
    }

    pub fn get_module_name(&self) -> &str {
        &self.name
    }
    pub fn set_module_name<S: ToString>(&mut self, name: S) {
        self.name = name.to_string();
    }
    pub fn register<T: WasmItem>(&mut self, item: T) {
        item.register(self);
    }

    pub fn import_groups(&self) -> BTreeMap<&WasmExternalName, Vec<&ExternalFunctionType>> {
        let mut groups = BTreeMap::new();
        for f in self.externals.values() {
            groups.entry(&f.external_package).or_insert_with(Vec::new).push(f);
        }
        groups
    }

    pub fn register_data(&mut self, item: DataItem) -> Option<DataItem> {
        self.data.insert(item.symbol.to_string(), item)
    }
    pub fn register_global(&mut self, global: WasmVariable) {
        self.globals.insert(global.symbol.to_string(), global);
    }
}
