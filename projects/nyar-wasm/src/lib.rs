#![feature(associated_type_defaults)]
// #![deny(missing_debug_implementations, missing_copy_implementations)]
// #![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]

pub mod helpers;
mod modules;
mod operations;
mod runner;
mod types;
mod values;

pub use crate::{
    modules::ModuleBuilder,
    types::{
        array::ArrayType,
        external::{ExternalRegister, ExternalType},
        structure::{FieldType, StructureType},
        TypeItem, TypeRegister,
    },
};
