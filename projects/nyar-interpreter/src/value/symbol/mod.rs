use super::*;
use crate::engine::ModuleInstance;
use std::sync::Arc;

pub enum Symbol {
    Module(Arc<ModuleInstance>),
    Variable,
    Class,
}
