//! 整数字面量的唯一定型规则。
//!
//! 字面量在 HIR 中保持未定型：`HirLiteral::Integer(i128)` 精确携带符号与量级，
//! 覆盖完整 `u64` 正半域。宽度与有符号性只在上下文已知处结算一次：
//! - 上下文给出具体整数类型（含 §4.10 B 的形参/返回/字段/容器约束）时，
//!   按该类型检查范围并产出其 64 位位模式；
//! - 上下文未给出（或给出非整数目标）时，按「能容纳该值的最小有符号宽度，
//!   仅当有符号 64 位装不下才落到 `u64`」给默认宽度（§4.14 的默认规则）。
//!
//! 该规则由 HIR 拥有；MIR 只能调用本模块取得已冻结的「常量位模式 + 类型」，
//! 不得在 lowering 侧另写默认宽度或范围判断。

use crate::compiler::types::hir::ValkyrieType;

/// 未定型整数字面量的默认宽度：`i32` → `i64` → `u64`（有符号优先）。
pub(crate) fn default_integer_type(value: i128) -> ValkyrieType {
    if (i32::MIN as i128..=i32::MAX as i128).contains(&value) {
        return ValkyrieType::Integer32 { signed: true };
    }

    if (i64::MIN as i128..=i64::MAX as i128).contains(&value) {
        return ValkyrieType::Integer64 { signed: true };
    }

    // 构造处已保证量级不超过 `u64::MAX`，且越过 `i64::MAX` 的值必为非负。
    ValkyrieType::Integer64 { signed: false }
}

/// 值是否落在具体整数类型的可表示范围内；非整数类型返回 `None`。
fn integer_type_fits(value: i128, ty: &ValkyrieType) -> Option<bool> {
    let fits = match *ty {
        ValkyrieType::Integer8 { signed: true } => (i8::MIN as i128..=i8::MAX as i128).contains(&value),
        ValkyrieType::Integer8 { signed: false } => (0..=u8::MAX as i128).contains(&value),
        ValkyrieType::Integer16 { signed: true } => (i16::MIN as i128..=i16::MAX as i128).contains(&value),
        ValkyrieType::Integer16 { signed: false } => (0..=u16::MAX as i128).contains(&value),
        ValkyrieType::Integer32 { signed: true } => (i32::MIN as i128..=i32::MAX as i128).contains(&value),
        ValkyrieType::Integer32 { signed: false } => (0..=u32::MAX as i128).contains(&value),
        ValkyrieType::Integer64 { signed: true } => (i64::MIN as i128..=i64::MAX as i128).contains(&value),
        ValkyrieType::Integer64 { signed: false } => (0..=u64::MAX as i128).contains(&value),
        ValkyrieType::Integer128 { signed: true } => (i128::MIN..=i128::MAX).contains(&value),
        // 字面量量级上界是 `u64`，任何非负值都可表示；负值在无符号 `u128` 中不可表示。
        ValkyrieType::Integer128 { signed: false } => value >= 0,
        _ => return None,
    };
    Some(fits)
}

/// 结算结果：`bits` 是该整数类型下的 64 位位模式（无符号值按补码存位），
/// `ty` 是冻结后的类型；二者必须一起消费，不得只取其一。
pub(crate) fn settle_integer(value: i128, expected: Option<&ValkyrieType>) -> Result<(i64, ValkyrieType), String> {
    if let Some(expected) = expected
        && let Some(fits) = integer_type_fits(value, expected)
    {
        if !fits {
            return Err(format!("integer literal `{value}` is out of range `{}`", type_name(expected)));
        }

        return Ok((value as i64, expected.clone()));
    }

    let ty = default_integer_type(value);
    Ok((value as i64, ty))
}

fn type_name(ty: &ValkyrieType) -> String {
    match ty {
        ValkyrieType::Integer8 { signed: true } => "i8".to_string(),
        ValkyrieType::Integer8 { signed: false } => "u8".to_string(),
        ValkyrieType::Integer16 { signed: true } => "i16".to_string(),
        ValkyrieType::Integer16 { signed: false } => "u16".to_string(),
        ValkyrieType::Integer32 { signed: true } => "i32".to_string(),
        ValkyrieType::Integer32 { signed: false } => "u32".to_string(),
        ValkyrieType::Integer64 { signed: true } => "i64".to_string(),
        ValkyrieType::Integer64 { signed: false } => "u64".to_string(),
        ValkyrieType::Integer128 { signed: true } => "i128".to_string(),
        ValkyrieType::Integer128 { signed: false } => "u128".to_string(),
        other => format!("{other:?}"),
    }
}
