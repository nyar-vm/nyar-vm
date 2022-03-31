#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

mod functions;
pub mod helpers;
mod modules;
mod operations;
mod runner;
mod types;
mod values;
pub use crate::{
    functions::{FunctionBody, FunctionType, ParameterType},
    modules::ModuleBuilder,
    operations::{Operation, VariableKind},
    symbols::WasmSymbol,
    types::{
        array::ArrayType,
        external::{ExternalRegister, ExternalType},
        functional::FunctionRegister,
        structure::{FieldType, StructureType},
        TypeItem, TypeRegister, WasmType,
    },
    values::{global::GlobalRegister, variable::WasmVariable, WasmValue},
};
mod symbols;
