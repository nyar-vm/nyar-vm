//! PowerShell evaluator thin adapter — delegates to `rusty-powershell::evaluate_powershell_source`.

use std::{any::Any, collections::HashMap, fmt, sync::Arc};

use rusty_powershell::{PowerShellValue, evaluate_powershell_source};

use crate::guest::{GuestEnv, GuestError, GuestValue, GuestValueHandle};

/// PowerShell-owned guest value.
#[derive(Debug, Clone)]
pub struct PowerShellHostValue(PowerShellValue);

impl fmt::Display for PowerShellHostValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_value())
    }
}

impl GuestValue for PowerShellHostValue {
    fn language(&self) -> &'static str {
        "powershell"
    }

    fn type_name(&self) -> &'static str {
        match self.0 {
            PowerShellValue::Null => "null",
            PowerShellValue::Bool(_) => "boolean",
            PowerShellValue::Int(_) => "integer",
            PowerShellValue::Float(_) => "number",
            PowerShellValue::String(_) => "string",
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self.0 {
            PowerShellValue::Int(value) => Some(value),
            PowerShellValue::Float(value) => Some(value as i64),
            PowerShellValue::Bool(value) => Some(i64::from(value)),
            PowerShellValue::Null | PowerShellValue::String(_) => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            PowerShellValue::Float(value) => Some(value),
            PowerShellValue::Int(value) => Some(value as f64),
            PowerShellValue::Null | PowerShellValue::Bool(_) | PowerShellValue::String(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        Some(match self.0 {
            PowerShellValue::Bool(value) => value,
            PowerShellValue::Null => false,
            PowerShellValue::Int(value) => value != 0,
            PowerShellValue::Float(value) => value != 0.0,
            PowerShellValue::String(ref value) => !value.is_empty(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Wrap a PowerShell interpreter value as a guest handle (host-side input).
pub fn powershell_host_value(value: PowerShellValue) -> GuestValueHandle {
    Arc::new(PowerShellHostValue(value))
}

/// Evaluate a PowerShell script via the language-owned interpreter.
pub fn evaluate_powershell_script(source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError> {
    let mut ps_env: HashMap<String, PowerShellValue> = env
        .iter()
        .map(|(key, value)| Ok((key.clone(), powershell_from_handle(value)?)))
        .collect::<Result<_, GuestError>>()?;

    let result = evaluate_powershell_source(source, &mut ps_env).map_err(|error| GuestError::new("powershell", error))?;

    env.clear();
    for (key, value) in ps_env {
        env.insert(key, powershell_host_value(value));
    }

    Ok(powershell_host_value(result))
}

fn powershell_from_handle(value: &GuestValueHandle) -> Result<PowerShellValue, GuestError> {
    value.as_any().downcast_ref::<PowerShellHostValue>().map(|host| host.0.clone()).ok_or_else(|| {
        GuestError::new("powershell", format!("环境值属于 `{}`，不能作为 PowerShell 值使用", value.language()))
    })
}
