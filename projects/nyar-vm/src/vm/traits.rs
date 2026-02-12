use crate::vm::value::{Value, WitnessTable};
use crate::vm::core::NyarVM;
use nyar_types::VmError;
use std::fmt::Debug;
use std::any::Any;

pub trait NyarObject {
    fn get_field(&self, name: &str) -> Option<Value>;
    fn set_field(&mut self, name: &str, value: Value) -> Result<(), VmError>;
}

pub type ExternFunc = fn(&mut NyarVM, &[Value]) -> Result<Value, VmError>;

pub trait RuntimeProvider: Send + Sync + Debug + Any {
    fn resolve(&self, name: &str) -> Option<ExternFunc>;
    fn get_intrinsic(&self, id: u32) -> Option<ExternFunc>;
}

#[derive(Debug, Default)]
pub struct NoopRuntime;

impl RuntimeProvider for NoopRuntime {
    fn resolve(&self, _name: &str) -> Option<ExternFunc> {
        None
    }
    fn get_intrinsic(&self, _id: u32) -> Option<ExternFunc> {
        None
    }
}

pub fn get_witness_table(module_idx: usize, methods: Vec<u16>) -> WitnessTable {
    WitnessTable {
        module_idx,
        methods,
    }
}
