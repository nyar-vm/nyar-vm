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
use std::sync::Arc;
use std::sync::RwLock;
use nyar_hir::{NyarError, Range};
use std::collections::HashMap;

pub type SharedModule = Arc<RwLock<ModuleInstance>>;