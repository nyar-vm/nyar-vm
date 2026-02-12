use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::Value;
use crate::ffi::FFIResult;

pub fn string_concat(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    let result = format!("{}{}", l, r);
    Ok(Value::string(result, &vm.gc))
}

pub fn string_len_bytes(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let s = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    Ok(Value::int(s.len() as i64))
}

pub fn string_len_chars(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let s = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    Ok(Value::int(s.chars().count() as i64))
}

pub fn string_eq(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str();
    let r = args[1].try_as_str();
    match (l, r) {
        (Some(l), Some(r)) => Ok(Value::bool(l == r)),
        _ => Ok(Value::bool(false)),
    }
}

pub fn string_ne(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str();
    let r = args[1].try_as_str();
    match (l, r) {
        (Some(l), Some(r)) => Ok(Value::bool(l != r)),
        _ => Ok(Value::bool(true)),
    }
}

pub fn string_lt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l < r))
}

pub fn string_le(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l <= r))
}

pub fn string_gt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l > r))
}

pub fn string_ge(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l >= r))
}

pub fn string_substr(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let s = args[0].try_as_str().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let start = args[1].try_as_int().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", args[1].tag()) }))? as usize;
    let len = args[2].try_as_int().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", args[2].tag()) }))? as usize;
    
    let end = start.saturating_add(len);
    let end = end.min(s.len());
    let sub = if start <= end { s[start..end].to_string() } else { String::new() };
    Ok(Value::string(sub, &vm.gc))
}
