use crate::vm::value::Value;
use crate::vm::VmError;

#[derive(Clone)]
pub struct HandlerFrame {
    pub catch_chunk: usize,
    pub frame_depth: usize,
}

pub fn perform_effect_internal(
    vm: &mut crate::vm::interpreter::NyarVM,
    module_idx: usize,
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
    if name == "add" {
        let a = args.get(0).cloned().unwrap_or(Value::null());
        let b = args.get(1).cloned().unwrap_or(Value::null());
        let res = unsafe {
            match (a.tag, b.tag) {
                (crate::vm::value::ValueTag::Int, crate::vm::value::ValueTag::Int) => {
                    Value::int(a.as_int() + b.as_int())
                }
                (crate::vm::value::ValueTag::Float, crate::vm::value::ValueTag::Float) => {
                    Value::float(a.as_float() + b.as_float())
                }
                (crate::vm::value::ValueTag::String, crate::vm::value::ValueTag::String) => {
                    let mut s = a.as_string().clone();
                    s.push_str(b.as_string());
                    Value::string(s)
                }
                _ => Value::null(),
            }
        };
        return Ok(Some(res));
    }
    if name.contains("Token::") {
        let variant_name = name.split("::").last().unwrap_or("");
        // Find Token class
        let class_idx = vm.modules[module_idx]
            .classes
            .iter()
            .position(|c| c.name.ends_with("::Token") || c.name == "Token");
        if let Some(idx) = class_idx {
            let idx = idx as u16;
            let val = args.get(0).cloned().unwrap_or(Value::null());
            // Token enum has __variant__ and one field (u or x) mapped to _0
            let fields = vec![Value::string(variant_name.to_string()), val];
            let obj = Value::object(idx, fields);
            return Ok(Some(obj));
        }
    }
    vm.log("Traceback (most recent call last):");
    vm.log(&format!("UnhandledEffect: {}", name));
    Err(VmError::UnhandledEffect(name))
}
