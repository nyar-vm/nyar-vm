mod context;
mod instance;
mod manager;

pub use self::{
    context::{DefaultDecimalHandler, DefaultIntegerHandler, NyarContext, NyarIndexSystem, NYAR_CONTEXT_PRESET},
    instance::ModuleInstance,
    manager::ModuleManager,
};
use crate::{
    value::{function::NyarFunction, variable::Variable},
    Result,
};
use nyar_hir::{NyarError, Range};
use std::collections::HashMap;
