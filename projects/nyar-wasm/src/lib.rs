#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

mod array;
mod data;
mod external;
mod functions;
pub mod helpers;
mod modules;
mod operations;
mod symbols;
mod types;

mod structures;
mod values;
pub use crate::{
    array::ArrayType,
    data::DataItem,
    external::ExternalType,
    functions::{FunctionBody, FunctionType, ParameterType},
    modules::WasmBuilder,
    operations::{
        branch::{EnumerationTable, JumpBranch, JumpCondition, JumpTable},
        Operation, VariableKind,
    },
    structures::{FieldType, StructureType},
    symbols::WasmSymbol,
    types::{external::ExternalSection, TypeSection, WasmType},
    values::{data::DataSection, global::GlobalSection, variable::WasmVariable, WasmValue},
};
