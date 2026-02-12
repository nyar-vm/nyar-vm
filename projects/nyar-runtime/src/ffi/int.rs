use crate::ffi::FFIResult;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;

pub fn i32_add(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int(lhs.wrapping_add(rhs) as i64))
}

pub fn i32_sub(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int(lhs.wrapping_sub(rhs) as i64))
}

pub fn i32_mul(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int(lhs.wrapping_mul(rhs) as i64))
}

pub fn i32_div_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_div(rhs);
    Ok(Value::int(r as i64))
}

pub fn i32_div_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_div(rhs);
    Ok(Value::int(r as i64))
}

pub fn i32_rem_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_rem(rhs);
    Ok(Value::int(r as i64))
}

pub fn i32_rem_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_rem(rhs);
    Ok(Value::int(r as i64))
}

pub fn i32_and(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int((lhs & rhs) as i64))
}

pub fn i32_or(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int((lhs | rhs) as i64))
}

pub fn i32_xor(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int((lhs ^ rhs) as i64))
}

pub fn i32_shl(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.wrapping_shl(rhs) as i64))
}

pub fn i32_shr_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.wrapping_shr(rhs) as i64))
}

pub fn i32_shr_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.wrapping_shr(rhs) as i64))
}

pub fn i32_not(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i32;
    Ok(Value::int((!v) as i64))
}

pub fn i32_neg(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i32;
    Ok(Value::int(v.wrapping_neg() as i64))
}

pub fn i32_eq(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::bool(lhs == rhs))
}

pub fn i32_ne(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::bool(lhs != rhs))
}

pub fn i32_lt_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::bool(lhs < rhs))
}

pub fn i32_lt_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::bool(lhs < rhs))
}

pub fn i32_le_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::bool(lhs <= rhs))
}

pub fn i32_le_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::bool(lhs <= rhs))
}

pub fn i32_gt_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::bool(lhs > rhs))
}

pub fn i32_gt_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::bool(lhs > rhs))
}

pub fn i32_ge_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::bool(lhs >= rhs))
}

pub fn i32_ge_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::bool(lhs >= rhs))
}

pub fn i32_to_f32_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i32;
    Ok(Value::f32(v as f32))
}

pub fn i32_to_f32_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as u32;
    Ok(Value::f32(v as f32))
}

pub fn i32_to_f64_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i32;
    Ok(Value::float(v as f64))
}

pub fn i32_to_f64_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as u32;
    Ok(Value::float(v as f64))
}

pub fn i32_extend64_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i32;
    Ok(Value::int(v as i64))
}

pub fn i32_extend64_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as u32;
    Ok(Value::int(v as u64 as i64))
}

pub fn i32_trunc64_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i64;
    Ok(Value::int(v as i32 as i64))
}

pub fn i32_trunc64_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as u64;
    Ok(Value::int(v as u32 as i64))
}

pub fn i32_add_sat_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int(lhs.saturating_add(rhs) as i64))
}

pub fn i32_add_sat_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.saturating_add(rhs) as i64))
}

pub fn i32_sub_sat_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i32;
    let rhs = args[1].as_int() as i32;
    Ok(Value::int(lhs.saturating_sub(rhs) as i64))
}

pub fn i32_sub_sat_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u32;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.saturating_sub(rhs) as i64))
}

// I64
pub fn i64_add(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs.wrapping_add(rhs)))
}

pub fn i64_sub(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs.wrapping_sub(rhs)))
}

pub fn i64_mul(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs.wrapping_mul(rhs)))
}

pub fn i64_div_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_div(rhs);
    Ok(Value::int(r))
}

pub fn i64_div_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_div(rhs);
    Ok(Value::int(r as i64))
}

pub fn i64_rem_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_rem(rhs);
    Ok(Value::int(r))
}

pub fn i64_rem_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    if rhs == 0 {
        return Err(vm.error(nyar_types::VmErrorKind::DivisionByZero));
    }
    let (r, _) = lhs.overflowing_rem(rhs);
    Ok(Value::int(r as i64))
}

pub fn i64_and(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs & rhs))
}

pub fn i64_or(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs | rhs))
}

pub fn i64_xor(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs ^ rhs))
}

pub fn i64_shl(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.wrapping_shl(rhs)))
}

pub fn i64_shr_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.wrapping_shr(rhs)))
}

pub fn i64_shr_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u32;
    Ok(Value::int(lhs.wrapping_shr(rhs) as i64))
}

pub fn i64_not(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i64;
    Ok(Value::int(!v))
}

pub fn i64_neg(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i64;
    Ok(Value::int(v.wrapping_neg()))
}

pub fn i64_eq(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::bool(lhs == rhs))
}

pub fn i64_ne(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::bool(lhs != rhs))
}

pub fn i64_lt_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::bool(lhs < rhs))
}

pub fn i64_lt_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    Ok(Value::bool(lhs < rhs))
}

pub fn i64_le_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::bool(lhs <= rhs))
}

pub fn i64_le_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    Ok(Value::bool(lhs <= rhs))
}

pub fn i64_gt_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::bool(lhs > rhs))
}

pub fn i64_gt_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    Ok(Value::bool(lhs > rhs))
}

pub fn i64_ge_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::bool(lhs >= rhs))
}

pub fn i64_ge_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    Ok(Value::bool(lhs >= rhs))
}

pub fn i64_to_f32_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i64;
    Ok(Value::f32(v as f32))
}

pub fn i64_to_f32_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as u64;
    Ok(Value::f32(v as f32))
}

pub fn i64_to_f64_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as i64;
    Ok(Value::float(v as f64))
}

pub fn i64_to_f64_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let v = args[0].as_int() as u64;
    Ok(Value::float(v as f64))
}

pub fn i64_add_sat_s(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as i64;
    let rhs = args[1].as_int() as i64;
    Ok(Value::int(lhs.saturating_add(rhs)))
}

pub fn i64_add_sat_u(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let lhs = args[0].as_int() as u64;
    let rhs = args[1].as_int() as u64;
    Ok(Value::int(lhs.saturating_add(rhs) as i64))
}
