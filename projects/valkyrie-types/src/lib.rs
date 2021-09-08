#![feature(trivial_bounds)]

pub use self::{
    builtin::result::{ValkyrieFailure, ValkyrieSuccess},
    types::{
        atomic_type::ValkyrieAtomicType, class_type::ValkyrieClassType, literal_type::ValkyrieLiteralType,
        tuple_type::ValkyrieClass, union_type::ValkyrieUnionType, variant_type::ValkyrieVariantType, ValkyrieTypeLegacy,
    },
    values::ValkyrieValue,
};

mod builtin;
mod codegen;
mod functions;
mod types;
mod values;
// #[cfg(test)]
pub mod testing;
