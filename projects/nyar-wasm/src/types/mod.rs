use crate::helpers::{Id, WasmBuilder, WasmDefineType, WasmEmitter, WasmOutput};
use nyar_hir::{FieldType, StructureType};
use wast::{
    core::{ArrayType, StorageType, StructField, StructType, Type, TypeDef},
    token::{NameAnnotation, Span},
};

mod defs;

mod interface;
