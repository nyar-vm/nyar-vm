use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::Value;
use crate::ffi::FFIResult;

// F32 operations

pub fn f32_add(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() + rhs.as_f32();
    Ok(Value::f32(r))
}

pub fn f32_sub(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() - rhs.as_f32();
    Ok(Value::f32(r))
}

pub fn f32_mul(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() * rhs.as_f32();
    Ok(Value::f32(r))
}

pub fn f32_div(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r_val = rhs.as_f32();
    if r_val == 0.0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let r = lhs.as_f32() / r_val;
    Ok(Value::f32(r))
}

pub fn f32_neg(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = -v.as_f32();
    Ok(Value::f32(r))
}

pub fn f32_eq(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() || !rhs.is_f32() {
        return Ok(Value::bool(false));
    }
    let r = lhs.as_f32() == rhs.as_f32();
    Ok(Value::bool(r))
}

pub fn f32_ne(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() || !rhs.is_f32() {
        return Ok(Value::bool(true));
    }
    let r = lhs.as_f32() != rhs.as_f32();
    Ok(Value::bool(r))
}

pub fn f32_lt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() < rhs.as_f32();
    Ok(Value::bool(r))
}

pub fn f32_le(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() <= rhs.as_f32();
    Ok(Value::bool(r))
}

pub fn f32_gt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() > rhs.as_f32();
    Ok(Value::bool(r))
}

pub fn f32_ge(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f32() >= rhs.as_f32();
    Ok(Value::bool(r))
}

pub fn f32_to_i32_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f32() as i32;
    Ok(Value::int(r as i64))
}

pub fn f32_to_i32_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f32() as u32;
    Ok(Value::int(r as i64))
}

pub fn f32_to_i64_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f32() as i64;
    Ok(Value::int(r))
}

pub fn f32_to_i64_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f32() as u64;
    Ok(Value::int(r as i64))
}

pub fn f32_to_f64(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f32() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f32() as f64;
    Ok(Value::float(r))
}

// F64 operations

pub fn f64_add(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() + rhs.as_f64();
    Ok(Value::float(r))
}

pub fn f64_sub(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() - rhs.as_f64();
    Ok(Value::float(r))
}

pub fn f64_mul(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() * rhs.as_f64();
    Ok(Value::float(r))
}

pub fn f64_div(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r_val = rhs.as_f64();
    if r_val == 0.0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let r = lhs.as_f64() / r_val;
    Ok(Value::float(r))
}

pub fn f64_neg(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = -v.as_f64();
    Ok(Value::float(r))
}

pub fn f64_eq(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() || !rhs.is_f64() {
        return Ok(Value::bool(false));
    }
    let r = lhs.as_f64() == rhs.as_f64();
    Ok(Value::bool(r))
}

pub fn f64_ne(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() || !rhs.is_f64() {
        return Ok(Value::bool(true));
    }
    let r = lhs.as_f64() != rhs.as_f64();
    Ok(Value::bool(r))
}

pub fn f64_lt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() < rhs.as_f64();
    Ok(Value::bool(r))
}

pub fn f64_le(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() <= rhs.as_f64();
    Ok(Value::bool(r))
}

pub fn f64_gt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() > rhs.as_f64();
    Ok(Value::bool(r))
}

pub fn f64_ge(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0];
    let rhs = args[1];
    if !lhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
    }
    if !rhs.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
    }
    let r = lhs.as_f64() >= rhs.as_f64();
    Ok(Value::bool(r))
}

pub fn f64_to_i32_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f64() as i32;
    Ok(Value::int(r as i64))
}

pub fn f64_to_i32_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f64() as u32;
    Ok(Value::int(r as i64))
}

pub fn f64_to_i64_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f64() as i64;
    Ok(Value::int(r))
}

pub fn f64_to_i64_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f64() as u64;
    Ok(Value::int(r as i64))
}

pub fn f64_to_f32(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0];
    if !v.is_f64() {
        return Err(vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
    }
    let r = v.as_f64() as f32;
    Ok(Value::f32(r))
}
