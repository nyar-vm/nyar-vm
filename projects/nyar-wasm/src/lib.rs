#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

mod array;
mod external;
mod functions;
pub mod helpers;
mod modules;
mod operations;
mod symbols;
mod types;

mod values;
pub use crate::{
    array::ArrayType,
    external::ExternalType,
    functions::{FunctionBody, FunctionType, ParameterType},
    modules::ModuleBuilder,
    operations::{branch::JumpBranch, Operation, VariableKind},
    symbols::WasmSymbol,
    types::{
        external::ExternalSection,
        functional::FunctionSection,
        structure::{FieldType, StructureType},
        TypeItem, TypeSection, WasmType,
    },
    values::{global::GlobalSection, variable::WasmVariable, WasmValue},
};
