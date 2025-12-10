use crate::vm::value::Value;
use crate::vm::VmError;

#[derive(Clone)]
pub struct HandlerFrame {}

pub fn perform_effect_internal(
    _vm: &mut crate::vm::interpreter::NyarVM,
    name: String,
    _args: Vec<Value>,
) -> Result<Option<Value>, VmError> {
    if name == "throw" {
        return Err(VmError::UnhandledError);
    }
    if name == "await" {
        return Ok(None);
    }
    Err(VmError::UnhandledEffect(name))
}
