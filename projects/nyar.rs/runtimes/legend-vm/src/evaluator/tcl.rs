//! Tcl evaluator thin adapter — delegates to `rusty-tcl::evaluate_tcl_source`.

use std::{any::Any, collections::HashMap, fmt, sync::Arc};

use rusty_tcl::{TclValue, evaluate_tcl_source};

use crate::guest::{GuestEnv, GuestError, GuestValue, GuestValueHandle};

/// Tcl-owned guest value.
#[derive(Debug, Clone)]
pub struct TclHostValue(TclValue);

impl fmt::Display for TclHostValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_value())
    }
}

impl GuestValue for TclHostValue {
    fn language(&self) -> &'static str {
        "tcl"
    }

    fn type_name(&self) -> &'static str {
        match self.0 {
            TclValue::Null => "null",
            TclValue::Bool(_) => "boolean",
            TclValue::Int(_) => "integer",
            TclValue::Float(_) => "number",
            TclValue::String(_) => "string",
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self.0 {
            TclValue::Int(value) => Some(value),
            TclValue::Float(value) => Some(value as i64),
            TclValue::Bool(value) => Some(i64::from(value)),
            TclValue::Null | TclValue::String(_) => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            TclValue::Float(value) => Some(value),
            TclValue::Int(value) => Some(value as f64),
            TclValue::Null | TclValue::Bool(_) | TclValue::String(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        Some(match self.0 {
            TclValue::Bool(value) => value,
            TclValue::Null => false,
            TclValue::Int(value) => value != 0,
            TclValue::Float(value) => value != 0.0,
            TclValue::String(ref value) => !value.is_empty(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Wrap a Tcl interpreter value as a guest handle (host-side input).
pub fn tcl_host_value(value: TclValue) -> GuestValueHandle {
    Arc::new(TclHostValue(value))
}

/// Evaluate a Tcl script via the language-owned interpreter.
pub fn evaluate_tcl_script(source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError> {
    let mut tcl_env: HashMap<String, TclValue> =
        env.iter().map(|(key, value)| Ok((key.clone(), tcl_from_handle(value)?))).collect::<Result<_, GuestError>>()?;

    let result = evaluate_tcl_source(source, &mut tcl_env).map_err(|error| GuestError::new("tcl", error))?;

    env.clear();
    for (key, value) in tcl_env {
        env.insert(key, tcl_host_value(value));
    }

    Ok(tcl_host_value(result))
}

fn tcl_from_handle(value: &GuestValueHandle) -> Result<TclValue, GuestError> {
    value
        .as_any()
        .downcast_ref::<TclHostValue>()
        .map(|host| host.0.clone())
        .ok_or_else(|| GuestError::new("tcl", format!("环境值属于 `{}`，不能作为 Tcl 值使用", value.language())))
}
