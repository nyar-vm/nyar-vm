#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
mod flags;

mod context;
mod instance;
mod manager;
mod optimized;

pub use self::{
    context::{DefaultDecimalHandler, DefaultIntegerHandler, NYAR_CONTEXT_PRESET, NyarContext, NyarIndexSystem},
    instance::ModuleInstance,
    manager::ModuleManager,
    flags::NyarReadWrite,
};

use std::collections::HashMap;
use std::sync::RwLock;
use nyar_hir::{NyarError, Range};
use shredder::{Scan, Scanner};
use shredder::marker::{GcSafe, GcDrop};
use shredder::plumbing::check_gc_drop;
use shredder::{Gc, GRwLock};
use crate::{
    Result,
    value::{function::NyarFunction, variable::Variable},
};




pub type SharedModule = Gc<RwLock<ModuleInstance>>;

