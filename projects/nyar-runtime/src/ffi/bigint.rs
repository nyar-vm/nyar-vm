use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::{BigInt, Value};
use num_bigint::{BigInt as NativeBigInt, Sign};
use crate::ffi::FFIResult;

pub fn bigint_const(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let sign_val = args[0].as_int() as u8;
    let bytes = args[1].try_as_bytes().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Bytes".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    
    let sign = if sign_val == 0 { Sign::Plus } else { Sign::Minus };
    let bi = NativeBigInt::from_bytes_le(sign, bytes);
    Ok(Value::bigint(BigInt(bi), &vm.gc))
}

pub fn bigint_add(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    let res = BigInt(&l.0 + &r.0);
    Ok(Value::bigint(res, &vm.gc))
}

pub fn bigint_sub(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    let res = BigInt(&l.0 - &r.0);
    Ok(Value::bigint(res, &vm.gc))
}

pub fn bigint_mul(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    let res = BigInt(&l.0 * &r.0);
    Ok(Value::bigint(res, &vm.gc))
}

pub fn bigint_div(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    if r.0 == NativeBigInt::from(0) {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let res = BigInt(&l.0 / &r.0);
    Ok(Value::bigint(res, &vm.gc))
}

pub fn bigint_mod(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    if r.0 == NativeBigInt::from(0) {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let res = BigInt(&l.0 % &r.0);
    Ok(Value::bigint(res, &vm.gc))
}

pub fn bigint_neg(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let bi = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let res = BigInt(-&bi.0);
    Ok(Value::bigint(res, &vm.gc))
}

pub fn bigint_eq(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint();
    let r = args[1].try_as_bigint();
    match (l, r) {
        (Some(l), Some(r)) => Ok(Value::bool(l.0 == r.0)),
        _ => Ok(Value::bool(false)),
    }
}

pub fn bigint_ne(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint();
    let r = args[1].try_as_bigint();
    match (l, r) {
        (Some(l), Some(r)) => Ok(Value::bool(l.0 != r.0)),
        _ => Ok(Value::bool(true)),
    }
}

pub fn bigint_lt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l.0 < r.0))
}

pub fn bigint_le(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l.0 <= r.0))
}

pub fn bigint_gt(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l.0 > r.0))
}

pub fn bigint_ge(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let l = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let r = args[1].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[1].tag()) }))?;
    Ok(Value::bool(l.0 >= r.0))
}

pub fn bigint_to_i64(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let b = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let i = b.to_i64();
    Ok(Value::int(i))
}

pub fn bigint_from_i64(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let i = args[0].try_as_int().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    Ok(Value::bigint_from_i64(i, &vm.gc))
}

pub fn bigint_to_string(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let b = args[0].try_as_bigint().ok_or_else(|| vm.error(nyar_types::VmErrorKind::TypeMismatch { expected: "BigInt".to_string(), found: format!("{:?}", args[0].tag()) }))?;
    let s = b.0.to_string();
    Ok(Value::string(s, &vm.gc))
}
