#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

mod array;
mod data;
mod enumerates;
mod external;
mod functions;
pub mod helpers;
mod modules;
mod operations;
mod structures;
mod symbols;
mod tags;
mod types;
mod variants;

pub use self::enumerates::EnumerateType;

mod flags;
mod values;

pub use crate::{
    array::ArrayType,
    data::DataItem,
    external::ExternalFunctionType,
    flags::{EncodingType, FlagType},
    functions::{FunctionBody, FunctionSignature, FunctionType, WasmParameter},
    modules::{WasmBuilder, WasmItem},
    operations::{
        branch::{EnumerationTable, JumpBranch, JumpCondition, JumpTable},
        Operation, VariableKind,
    },
    structures::{FieldType, StructureItem, StructureType},
    symbols::{WasiName, WasmPublisher, WasmSymbol},
    types::{external::ImportSection, WasmType},
    values::{data::DataSection, variable::WasmVariable, WasmValue},
    variants::VariantType,
};
