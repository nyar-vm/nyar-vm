use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_vm::vm::value::{Value, ValueTag};
use nyar_gc::NyarGc;
use nyar_types::NyarError;
use oak_json::ast::JsonValue;
use std::collections::HashMap;

pub struct StdJsonParse;

impl FFIFunction for StdJsonParse {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature { params: vec![FFIType::String], ret: FFIType::Any })
    }

    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let json_str = args
            .get(0)
            .ok_or_else(|| NyarError::RuntimeError("Missing json string argument".to_string()))?
            .try_as_str()
            .ok_or_else(|| NyarError::RuntimeError("Argument must be a string".to_string()))?;

        let json_value = oak_json::parse(json_str).map_err(|e| NyarError::RuntimeError(format!("JSON parse error: {}", e)))?;

        Ok(json_to_nyar(json_value, &vm.gc))
    }
}

pub struct StdJsonStringify;

impl FFIFunction for StdJsonStringify {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature { params: vec![FFIType::Any], ret: FFIType::String })
    }

    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing argument".to_string()))?;
        let json_str = nyar_to_json_string(val);
        Ok(Value::string(json_str, &vm.gc))
    }
}

fn json_to_nyar(json: JsonValue, gc: &NyarGc) -> Value {
    match json {
        JsonValue::Null(_) => Value::null(),
        JsonValue::Boolean(b) => Value::bool(b.value),
        JsonValue::Number(n) => {
            if n.value.fract() == 0.0 {
                Value::int(n.value as i64)
            } else {
                Value::float(n.value)
            }
        }
        JsonValue::String(s) => Value::string(s.value, gc),
        JsonValue::Array(a) => {
            let items: Vec<Value> = a.elements.into_iter().map(|e| json_to_nyar(e, gc)).collect();
            Value::list(items, gc)
        }
        JsonValue::Object(o) => {
            let mut entries = HashMap::new();
            for field in o.fields {
                entries.insert(field.name.value, json_to_nyar(field.value, gc));
            }
            let val = Value::dyn_object(gc);
            unsafe {
                val.as_dyn_object_mut().entries = entries;
            }
            val
        }
    }
}

fn nyar_to_json_string(val: &Value) -> String {
    if val.is_float() {
        return val.as_float().to_string();
    }
    match val.tag() {
        ValueTag::Null => "null".to_string(),
        ValueTag::Bool => val.as_bool().to_string(),
        ValueTag::Int => val.as_int().to_string(),
        ValueTag::String => {
            if let Some(s) = val.try_as_str() {
                format!("\"{}\"", s.replace("\"", "\\\""))
            } else {
                "\"\"".to_string()
            }
        }
        ValueTag::List | ValueTag::Array => {
            let items = if val.tag() == ValueTag::List {
                unsafe { &val.as_list().items }
            } else {
                unsafe { &val.as_array().items }
            };
            let parts: Vec<String> = items.iter().map(nyar_to_json_string).collect();
            format!("[{}]", parts.join(","))
        }
        ValueTag::DynObject => {
            let obj = unsafe { val.as_dyn_object() };
            let parts: Vec<String> = obj
                .entries
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k.replace("\"", "\\\""), nyar_to_json_string(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        _ => "null".to_string(),
    }
}
