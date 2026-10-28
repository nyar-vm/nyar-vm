//! Primitive numeric operator intrinsic lowering —为没有显式 `imply` 的
//! primitive 数值类型（i32 / i64 / f32 / f64 等）自动注入 built-in operator callable。
//!
//! 调用点在 [`try_resolve_call`] 的 operator member 分支末尾，当
//! `try_resolve_instance_method` 返回 `None` 时触发。fallback 会检查 receiver 类型
//! 是否为 primitive numeric、operator 名是否在支持集内，然后构造一个带
//! synthetic [`ItemInstanceId`] 的 [`HirResolvedCall`]。后续 Semantic MIR lowering
//! 会把它当作普通 member call 处理，而 [`crate::compiler::mir::ssa::lowerer::MirLowerer`]
//! 会在 lowering 后扫描这些 synthetic instance，为它们自动登记
//! 带 `wasm_operator` link 的 external call contract。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::type_relation::TypeRelationContext,
    mir::AggregateFieldIndex,
    types::{
        Identifier,
        hir::{HirCallArgument, HirCallableDomain, HirExpr, HirResolvedCall, ValkyrieType},
    },
};

use nyar_types::{ItemInstanceId, NamePath};

// ── Synthetic instance 分配基数 ─────────────────────────────────────────────
//
// 所有 primitive operator synthetic instance 从此基数起，每项偏移 1：
//   synthetic_index = PRIMITIVE_OPERATOR_INSTANCE_BASE + type_offset * 100 + op_offset
//
// 取值 u32::MAX / 2 保证不与用户显式声明的 instance 冲突。
pub const PRIMITIVE_OPERATOR_INSTANCE_BASE: u32 = 1u32 << 30;

/// 返回 `Some((type_offset, type_name))` 当且仅当 `ty` 是 primitive numeric / boolean。
pub fn primitive_numeric_type_offset(ty: &ValkyrieType) -> Option<(u32, &'static str)> {
    match ty {
        ValkyrieType::Boolean => Some((0, "boolean")),
        ValkyrieType::Integer8 { .. } => Some((1, "i8")),
        ValkyrieType::Integer16 { .. } => Some((2, "i16")),
        ValkyrieType::Integer32 { .. } => Some((3, "i32")),
        ValkyrieType::Integer64 { .. } => Some((4, "i64")),
        ValkyrieType::Integer128 { .. } => Some((5, "i128")),
        ValkyrieType::Float32 => Some((6, "f32")),
        ValkyrieType::Float64 => Some((7, "f64")),
        ValkyrieType::Named(name) => match name.as_str() {
            "boolean" | "Bool" | "bool" => Some((0, "boolean")),
            "i8" => Some((1, "i8")),
            "i16" => Some((2, "i16")),
            "i32" | "int" | "Int" => Some((3, "i32")),
            "i64" | "long" | "Long" => Some((4, "i64")),
            "i128" => Some((5, "i128")),
            "f32" | "float" | "Float" => Some((6, "f32")),
            "f64" | "double" | "Double" => Some((7, "f64")),
            _ => None,
        },
        _ => None,
    }
}

/// 根据 numeric 类型偏移返回 wasm 整数宽度前缀（i32 / i64）。
fn wasm_int_prefix(type_offset: u32) -> &'static str {
    match type_offset {
        4 => "i64", // Integer64
        _ => "i32", // Integer32 / Integer16 / Integer8 / Integer128 等
    }
}

/// 根据 numeric 类型偏移返回 wasm 浮点数前缀（f32 / f64）。
fn wasm_float_prefix(type_offset: u32) -> Option<&'static str> {
    match type_offset {
        6 => Some("f32"),
        7 => Some("f64"),
        _ => None,
    }
}

/// 返回 `Some((op_offset, wasm_opcode, operand_count, is_comparison))`。
/// comparison operator 返回 Bool 而非 numeric。
/// prefix `-` / `!` 是一元操作。
pub fn operator_info(name: &str, type_offset: u32) -> Option<(u32, &'static str, u8, bool)> {
    let float_prefix = wasm_float_prefix(type_offset);

    let (op_offset, suffix, nargs, is_cmp) = match name {
        // 算术
        "infix +" => (0u32, "add", 2, false),
        "infix -" => (1, "sub", 2, false),
        "infix *" => (2, "mul", 2, false),
        "infix /" => (3, "div_s", 2, false),
        "infix %" => (4, "rem_s", 2, false),
        // 比较
        "infix ==" => (10, "eq", 2, true),
        "infix !=" => (11, "ne", 2, true),
        "infix <" => (12, "lt_s", 2, true),
        "infix <=" => (13, "le_s", 2, true),
        "infix >" => (14, "gt_s", 2, true),
        "infix >=" => (15, "ge_s", 2, true),
        // 位操作（仅整数）
        "infix &" => (20, "and", 2, false),
        "infix |" => (21, "or", 2, false),
        "infix ^" => (22, "xor", 2, false),
        // 一元
        "prefix -" => (30, "neg", 1, false),
        "prefix !" => (31, "not", 1, false),
        _ => return None,
    };

    // float 类型修正：div_s → div，lt_s → lt，等
    let suffix = if float_prefix.is_some() {
        match suffix {
            "div_s" => "div",
            "rem_s" => "rem",
            "lt_s" => "lt",
            "le_s" => "le",
            "gt_s" => "gt",
            "ge_s" => "ge",
            other => other,
        }
    }
    else {
        suffix
    };

    // 构造完整助记符前缀
    let prefix = if let Some(fp) = float_prefix { fp } else { wasm_int_prefix(type_offset) };

    // 把前缀拼接为 'static —— 用 Box::leak
    let opcode_str = format!("{prefix}.{suffix}");
    let opcode: &'static str = Box::leak(opcode_str.into_boxed_str());

    Some((op_offset, opcode, nargs, is_cmp))
}

/// 构造 synthetic [`ItemInstanceId`]。
pub fn synthetic_operator_instance(type_offset: u32, op_offset: u32) -> Option<ItemInstanceId> {
    let index = PRIMITIVE_OPERATOR_INSTANCE_BASE.checked_add(type_offset.checked_mul(100)?)?.checked_add(op_offset)?;
    ItemInstanceId::from_index(index)
}

/// 根据 type_offset 构造底层 primitive 类型（将 Named 解析成具体变体）。
pub fn primitive_type_from_offset(type_offset: u32) -> Option<ValkyrieType> {
    Some(match type_offset {
        0 => ValkyrieType::Boolean,
        1 => ValkyrieType::Integer8 { signed: true },
        2 => ValkyrieType::Integer16 { signed: true },
        3 => ValkyrieType::Integer32 { signed: true },
        4 => ValkyrieType::Integer64 { signed: true },
        5 => ValkyrieType::Integer128 { signed: true },
        6 => ValkyrieType::Float32,
        7 => ValkyrieType::Float64,
        _ => return None,
    })
}

/// 构造 primitive numeric operator synthetic resolved call。
///
/// 失败条件：receiver 不是 primitive numeric、operator 不支持、参数数量不符。
#[allow(clippy::too_many_arguments)]
pub fn try_resolve_primitive_numeric_operator(
    first: &HirExpr,
    callee_name: &str,
    method_args: &[HirCallArgument],
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<nyar_types::Identifier>,
) -> Option<HirResolvedCall> {
    use crate::compiler::hir::overload::types::infer_scrutinee_type;

    // 1. 推断 receiver 类型
    let receiver_type = match infer_scrutinee_type(first, locals, struct_fields, singleton_names) {
        Some(t) => t,
        None => return None,
    };

    // 2. 检查是否是 primitive numeric
    let (type_offset, type_name) = primitive_numeric_type_offset(&receiver_type)?;

    // 3. 检查 operator 是否支持
    let (op_offset, _wasm_opcode, nargs, is_cmp) = operator_info(callee_name, type_offset)?;

    // 4. 检查参数数量（二元=1个额外参数，一元=0）
    let expected = nargs.saturating_sub(1) as usize; // receiver 已经占用一个
    if method_args.len() != expected {
        return None;
    }

    // 5. 构造 synthetic instance
    let instance = synthetic_operator_instance(type_offset, op_offset)?;

    // 6. 构造返回类型 —— 用底层 primitive 变体（canonical 层不接受 Named）
    let receiver_primitive = primitive_type_from_offset(type_offset)?;
    let return_type = if is_cmp { ValkyrieType::Boolean } else { receiver_primitive.clone() };

    // 7. 构造 symbol
    let symbol = NamePath::new(vec![
        Identifier::new("__builtin__"),
        Identifier::new("primitive"),
        Identifier::new(type_name),
        Identifier::new(callee_name),
    ]);

    // 8. 构造 parameter_types —— 也用底层 primitive 变体
    let mut parameter_types = vec![receiver_primitive.clone()];
    if expected > 0 {
        parameter_types.push(receiver_primitive);
    }

    Some(HirResolvedCall {
        declaration: None,
        instance: Some(instance),
        intrinsic: None, // 没有新的 IntrinsicId——通过 instance + type 特判
        trait_dispatch: None,
        sum_owner: None,
        sum_variant: None,
        symbol,
        domain: HirCallableDomain::Function,
        return_type,
        parameter_types,
        has_receiver: true,
        extractor_payload_type: None,
    })
}

/// 给定 synthetic instance 反查原始信息（供 Semantic lowering 使用）。
///
/// 返回 `Some((type_offset, operator_name, wasm_opcode))`。
pub fn decode_synthetic_operator_instance(instance: ItemInstanceId) -> Option<(u32, &'static str, &'static str)> {
    let index = instance.index();
    if index < PRIMITIVE_OPERATOR_INSTANCE_BASE {
        return None;
    }
    let local = index - PRIMITIVE_OPERATOR_INSTANCE_BASE;
    let type_offset = local / 100;
    let op_offset = local % 100;

    // 从 op_offset 反查 operator 名
    let op_name = match op_offset {
        0 => "infix +",
        1 => "infix -",
        2 => "infix *",
        3 => "infix /",
        4 => "infix %",
        10 => "infix ==",
        11 => "infix !=",
        12 => "infix <",
        13 => "infix <=",
        14 => "infix >",
        15 => "infix >=",
        20 => "infix &",
        21 => "infix |",
        22 => "infix ^",
        30 => "prefix -",
        31 => "prefix !",
        _ => return None,
    };
    let (_, opcode, _, _) = operator_info(op_name, type_offset)?;
    Some((type_offset, op_name, opcode))
}
