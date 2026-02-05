use crate::vm::value::Value;
use nyar_types::{EffectInfo, NyarError};

#[derive(Clone)]
pub struct HandlerFrame {
    pub module_idx: usize,
    pub catch_chunk: usize,
    pub frame_depth: usize,
}

pub fn perform_effect_internal(
    vm: &mut crate::vm::core::NyarVM,
    module_idx: usize,
    effect: EffectInfo,
    args: Vec<Value>,
) -> Result<Option<Value>, NyarError> {
    if effect.name.parts.len() == 1 {
        let name = &effect.name.parts[0];
        match name.as_str() {
            "LoggerEvent" | "print" => {
                for arg in &args {
                    vm.log(&format!("{}", arg));
                }
                return Ok(None);
            }
            "exit" => {
                let code = args.get(0).map(|v| v.as_int()).unwrap_or(0) as i32;
                std::process::exit(code);
            }
            "now" => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                return Ok(Some(Value::float(now)));
            }
            "get_env" => {
                if let Some(key) = args.get(0).and_then(|v| v.try_as_str()) {
                    if let Ok(val) = std::env::var(key) {
                        return Ok(Some(Value::string(val, &vm.gc)));
                    }
                }
                return Ok(Some(Value::null()));
            }
            "await" => {
                if let Some(val) = args.get(0) {
                    if val.is_closure() {
                        let res = vm.call_closure_sync(*val, vec![])?;
                        return Ok(Some(res));
                    }
                }
                return Ok(None);
            }
            "add" => {
                let a = args.get(0).cloned().unwrap_or(Value::null());
                let b = args.get(1).cloned().unwrap_or(Value::null());
                let res = match (a.tag(), b.tag()) {
                    (crate::vm::value::ValueTag::Int, crate::vm::value::ValueTag::Int) => {
                        Value::int(a.as_int() + b.as_int())
                    }
                    (crate::vm::value::ValueTag::F32, crate::vm::value::ValueTag::F32) => {
                        Value::f32(a.as_f32() + b.as_f32())
                    }
                    (crate::vm::value::ValueTag::F64, crate::vm::value::ValueTag::F64) => {
                        Value::float(a.as_f64() + b.as_f64())
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
            _ => {}
        }
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
            let obj = Value::object(module_idx, idx, fields, &vm.gc);
            return Ok(Some(obj));
        }
    }
    vm.log("Traceback (most recent call last):");
    vm.log(&format!("UnhandledEffect: {} at source {} offset {}", effect.name, effect.location.source_id, effect.location.offset));
    Err(vm.error(nyar_types::VmErrorKind::UnhandledEffect(effect.name)))
}
