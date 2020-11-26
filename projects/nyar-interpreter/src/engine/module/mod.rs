mod context;
mod manager;
mod instance;

pub use self::{
    context::{DefaultDecimalHandler, DefaultIntegerHandler, NyarContext, NyarIndexSystem, NYAR_CONTEXT_PRESET},
    manager::ModuleManager,
    instance::ModuleInstance,
};
use crate::{
    value::{function::NyarFunction, variable::Variable},
    Result,
};
use nyar_hir::{NyarError, Range};
use std::{
    collections::HashMap,
    lazy::SyncLazy,
    rc::{Rc, Weak},
};

