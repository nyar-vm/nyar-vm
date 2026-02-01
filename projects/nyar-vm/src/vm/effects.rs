use crate::vm::value::Value;
use crate::vm::VmError;
use nyar_types::{EffectInfo, QualifiedName};

#[derive(Clone)]
pub struct HandlerFrame {
    pub catch_chunk: usize,
    pub frame_depth: usize,
}

pub fn perform_effect_internal(
    vm: &mut crate::vm::core::NyarVM,
    module_idx: usize,
    effect: EffectInfo,
    args: Vec<Value>,
) -> Result<Option<Value>, VmError> {
    let name = effect.name.to_string();
    if name == "LoggerEvent" || name == "print" {
        for arg in &args {
            vm.log(&format!("{}", arg));
        }
        return Ok(None);
    }
    if name == "exit" {
        let code = args.get(0).map(|v| v.as_int()).unwrap_or(0) as i32;
        std::process::exit(code);
    }
    if name == "now" {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        return Ok(Some(Value::float(now)));
    }
    if name == "get_env" {
        if let Some(key) = args.get(0).and_then(|v| v.try_as_str()) {
            if let Ok(val) = std::env::var(key) {
                return Ok(Some(Value::string(val, &vm.gc)));
            }
        }
        return Ok(Some(Value::null()));
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
        let res = match (a.tag(), b.tag()) {
            (crate::vm::value::ValueTag::Int, crate::vm::value::ValueTag::Int) => {
                Value::int(a.as_int() + b.as_int())
            }
            (crate::vm::value::ValueTag::Float, crate::vm::value::ValueTag::Float) => {
                Value::float(a.as_float() + b.as_float())
            }
            (crate::vm::value::ValueTag::String, crate::vm::value::ValueTag::String) => {
                let mut s = a.try_as_str().unwrap_or("").to_string();
                s.push_str(b.try_as_str().unwrap_or(""));
                Value::string(s, &vm.gc)
            }
            _ => Value::null(),
        };
        return Ok(Some(res));
    }
    // Handle Token variants using QualifiedName
    if effect.name.parts.len() >= 2 && effect.name.parts[effect.name.parts.len() - 2] == "Token" {
        let variant_name = effect.name.parts.last().map(|s| s.as_str()).unwrap_or("");
        // Find Token class
        let class_idx = vm.modules[module_idx]
            .classes
            .iter()
            .position(|c| {
                c.name.parts.last().map(|s| s.as_str()) == Some("Token")
            });
        if let Some(idx) = class_idx {
            let idx = idx as u16;
            let val = args.get(0).cloned().unwrap_or(Value::null());
            // Token enum has __variant__ and one field (u or x) mapped to _0
            let fields = vec![Value::string(variant_name.to_string(), &vm.gc), val];
            let obj = Value::object(idx, fields, &vm.gc);
            return Ok(Some(obj));
        }
    }
    vm.log("Traceback (most recent call last):");
    vm.log(&format!("UnhandledEffect: {} at source {} offset {}", effect.name, effect.location.source_id, effect.location.offset));
    Err(VmError::UnhandledEffect(effect.name))
}
