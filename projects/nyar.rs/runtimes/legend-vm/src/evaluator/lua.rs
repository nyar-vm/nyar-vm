//! Lua evaluator thin adapter — delegates to `rusty-lua::evaluate_lua_source`.

use std::{any::Any, collections::HashMap, fmt, sync::Arc};

use rusty_lua::{LuaValue, evaluate_lua_source};

use crate::guest::{GuestEnv, GuestError, GuestValue, GuestValueHandle};

/// Lua-owned guest value: the handle keeps the interpreter value, including table identity.
#[derive(Debug, Clone)]
pub struct LuaHostValue(LuaValue);

impl fmt::Display for LuaHostValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string_value())
    }
}

impl GuestValue for LuaHostValue {
    fn language(&self) -> &'static str {
        "lua"
    }

    fn type_name(&self) -> &'static str {
        match self.0 {
            LuaValue::Null => "nil",
            LuaValue::Bool(_) => "boolean",
            LuaValue::Int(_) => "integer",
            LuaValue::Float(_) => "number",
            LuaValue::String(_) => "string",
            LuaValue::Table(_) => "table",
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self.0 {
            LuaValue::Int(value) => Some(value),
            LuaValue::Float(value) => Some(value as i64),
            LuaValue::Bool(value) => Some(i64::from(value)),
            LuaValue::Null | LuaValue::String(_) | LuaValue::Table(_) => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            LuaValue::Float(value) => Some(value),
            LuaValue::Int(value) => Some(value as f64),
            LuaValue::Null | LuaValue::Bool(_) | LuaValue::String(_) | LuaValue::Table(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        Some(self.0.to_bool())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Wrap a Lua interpreter value as a guest handle (host-side input).
pub fn lua_host_value(value: LuaValue) -> GuestValueHandle {
    Arc::new(LuaHostValue(value))
}

/// Evaluate a Lua script via the language-owned interpreter.
pub fn evaluate_lua_script(source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError> {
    let mut lua_env: HashMap<String, LuaValue> = env
        .iter()
        .map(|(key, value)| Ok((key.clone(), lua_from_handle(value)?)))
        .collect::<Result<_, GuestError>>()?;

    let result = evaluate_lua_source(source, &mut lua_env).map_err(|error| GuestError::new("lua", error))?;

    env.clear();
    for (key, value) in lua_env {
        env.insert(key, lua_host_value(value));
    }

    Ok(lua_host_value(result))
}

fn lua_from_handle(value: &GuestValueHandle) -> Result<LuaValue, GuestError> {
    value
        .as_any()
        .downcast_ref::<LuaHostValue>()
        .map(|host| host.0.clone())
        .ok_or_else(|| GuestError::new("lua", format!("环境值属于 `{}`，不能作为 Lua 值使用", value.language())))
}
