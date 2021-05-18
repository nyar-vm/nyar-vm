use nyar_error::NyarError;

pub use self::{
    nyar_class::{CustomClass, NyarClass},
    traits::*,
    values::{NyarDict, NyarList, NyarValue},
};

pub mod modifiers;
mod nyar_class;
mod nyar_function;
mod nyar_primitive;
mod nyar_variants;
mod traits;
mod values;

pub type NyarResult<T = NyarValue> = Result<T, NyarError>;
