extern crate core;

use nyar_error::NyarError;

pub use self::{
    nyar_class::{CustomClass, NyarClass},
    traits::*,
    values::{AnyDict, AnyList, NyarValue},
};

pub mod modifiers;
mod nyar_class;
mod nyar_function;
mod nyar_primitive;
mod nyar_variants;
mod traits;
mod values;

pub type NyarResult<T = NyarValue> = Result<T, NyarError>;
