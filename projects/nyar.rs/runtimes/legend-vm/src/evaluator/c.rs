//! C evaluator thin adapter — delegates to `rusty-c::evaluate_c_source`.

use std::{any::Any, collections::HashMap, fmt, sync::Arc};

use rusty_c::{CValue, evaluate_c_source};

use crate::guest::{GuestEnv, GuestError, GuestValue, GuestValueHandle};

/// C-owned guest value.
#[derive(Debug, Clone)]
pub struct CHostValue(CValue);

impl fmt::Display for CHostValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_value())
    }
}

impl GuestValue for CHostValue {
    fn language(&self) -> &'static str {
        "c"
    }

    fn type_name(&self) -> &'static str {
        match self.0 {
            CValue::Null => "void",
            CValue::Bool(_) => "bool",
            CValue::Int(_) => "int",
            CValue::Float(_) => "double",
            CValue::String(_) => "string",
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self.0 {
            CValue::Int(value) => Some(value),
            CValue::Float(value) => Some(value as i64),
            CValue::Bool(value) => Some(i64::from(value)),
            CValue::Null | CValue::String(_) => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            CValue::Float(value) => Some(value),
            CValue::Int(value) => Some(value as f64),
            CValue::Null | CValue::Bool(_) | CValue::String(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        Some(match self.0 {
            CValue::Bool(value) => value,
            CValue::Null => false,
            CValue::Int(value) => value != 0,
            CValue::Float(value) => value != 0.0,
            CValue::String(ref value) => !value.is_empty(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Wrap a C interpreter value as a guest handle (host-side input).
pub fn c_host_value(value: CValue) -> GuestValueHandle {
    Arc::new(CHostValue(value))
}

/// Evaluate a C translation unit via the language-owned interpreter.
pub fn evaluate_c_script(source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError> {
    let mut c_env: HashMap<String, CValue> =
        env.iter().map(|(key, value)| Ok((key.clone(), c_from_handle(value)?))).collect::<Result<_, GuestError>>()?;

    let result = evaluate_c_source(source, &mut c_env).map_err(|error| GuestError::new("c", error))?;

    env.clear();
    for (key, value) in c_env {
        env.insert(key, c_host_value(value));
    }

    Ok(c_host_value(result))
}

fn c_from_handle(value: &GuestValueHandle) -> Result<CValue, GuestError> {
    value
        .as_any()
        .downcast_ref::<CHostValue>()
        .map(|host| host.0.clone())
        .ok_or_else(|| GuestError::new("c", format!("环境值属于 `{}`，不能作为 C 值使用", value.language())))
}
