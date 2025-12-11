use crate::vm::value::Value;
use crate::vm::VmError;

#[derive(Clone)]
pub struct HandlerFrame {
    pub catch_chunk: usize,
    pub frame_depth: usize,
}

pub fn perform_effect_internal(
    vm: &mut crate::vm::interpreter::NyarVM,
    name: String,
    args: Vec<Value>,
) -> Result<Option<Value>, VmError> {
    if name == "LoggerEvent" {
        if let Some(v) = args.last() {
            let msg = match v.tag {
                crate::vm::value::ValueTag::Int => format!("{}", unsafe { v.as_int() }),
                crate::vm::value::ValueTag::Float => format!("{}", unsafe { v.as_float() }),
                crate::vm::value::ValueTag::Bool => format!("{}", unsafe { v.as_bool() }),
                crate::vm::value::ValueTag::Null => "null".to_string(),
                crate::vm::value::ValueTag::String => unsafe { v.as_string().clone() },
                _ => "<unsupported>".to_string(),
            };
            vm.log(&msg);
        }
        return Ok(None);
    }
    if name == "throw" {
        vm.log("Traceback (most recent call last):");
        vm.log("UnhandledError");
        return Err(VmError::UnhandledError);
    }
    if name == "await" {
        return Ok(None);
    }
    vm.log("Traceback (most recent call last):");
    vm.log(&format!("UnhandledEffect: {}", name));
    Err(VmError::UnhandledEffect(name))
}
