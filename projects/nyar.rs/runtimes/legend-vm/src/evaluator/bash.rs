//! Bash evaluator thin adapter — delegates to `rusty-bash::evaluate_bash_source`.

use std::{any::Any, collections::HashMap, fmt, sync::Arc};

use rusty_bash::{BashValue, evaluate_bash_source};

use crate::guest::{GuestEnv, GuestError, GuestValue, GuestValueHandle};

/// Bash-owned guest value.
#[derive(Debug, Clone)]
pub struct BashHostValue(BashValue);

impl fmt::Display for BashHostValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_value())
    }
}

impl GuestValue for BashHostValue {
    fn language(&self) -> &'static str {
        "bash"
    }

    fn type_name(&self) -> &'static str {
        match self.0 {
            BashValue::Null => "null",
            BashValue::Bool(_) => "boolean",
            BashValue::Int(_) => "integer",
            BashValue::String(_) => "string",
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self.0 {
            BashValue::Int(value) => Some(value),
            BashValue::Bool(value) => Some(i64::from(value)),
            BashValue::Null | BashValue::String(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        Some(match self.0 {
            BashValue::Bool(value) => value,
            BashValue::Null => false,
            BashValue::Int(value) => value != 0,
            BashValue::String(ref value) => !value.is_empty(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Wrap a Bash interpreter value as a guest handle (host-side input).
pub fn bash_host_value(value: BashValue) -> GuestValueHandle {
    Arc::new(BashHostValue(value))
}

/// Evaluate a Bash script via the language-owned interpreter.
pub fn evaluate_bash_script(source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError> {
    let mut bash_env: HashMap<String, BashValue> = env
        .iter()
        .map(|(key, value)| Ok((key.clone(), bash_from_handle(value)?)))
        .collect::<Result<_, GuestError>>()?;

    let result = evaluate_bash_source(source, &mut bash_env).map_err(|error| GuestError::new("bash", error))?;

    env.clear();
    for (key, value) in bash_env {
        env.insert(key, bash_host_value(value));
    }

    Ok(bash_host_value(result))
}

fn bash_from_handle(value: &GuestValueHandle) -> Result<BashValue, GuestError> {
    value
        .as_any()
        .downcast_ref::<BashHostValue>()
        .map(|host| host.0.clone())
        .ok_or_else(|| GuestError::new("bash", format!("环境值属于 `{}`，不能作为 Bash 值使用", value.language())))
}
