//! Wasm physical representation of NyarType / AggregateLayout.
//!
//! Owns unique storage and value-type rules for the Wasm GC backend:
//! Semantic MIR `StorageKind`, ABI param/return bytes, and GC field-type bytes.
//! This is Wasm **physical representation** only — Semantic MIR remains the
//! language type authority; backend only chooses Wasm encoding.
//!
//! Zero-logic extract from the former monolithic lowerer (B4 seam).

use super::{ExecutableLoweringContext, LayoutId, MirFunction, MirStorageKind, NyarType, StorageKind, WASM_GC_ANYREF};
use crate::lowering::backends::wasm::gc::{wasm_gc_field_type_byte, wasm_js_glue_value_carrier_valtype};
use acorn_wasm::{VALTYPE_ANYREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64};
use nyar_types::AggregateLayout;
use std::collections::BTreeMap;

/// 闭包对象（合同 §5.3）的物理槽位：0 = funcref table 槽位（i32），1 = 捕获环境（anyref / null）。
pub(in crate::lowering::backends::wasm) const WASM_CLOSURE_CODE_FIELD: u32 = 0;
pub(in crate::lowering::backends::wasm) const WASM_CLOSURE_ENV_FIELD: u32 = 1;

pub(super) fn is_js_glue_host_string_type(ty: &NyarType) -> bool {
    // Only the explicit language encoding reaches the JS glue ABI. Nominal
    // names are not evidence of text semantics.
    matches!(ty, NyarType::Utf8)
}

pub(super) fn mir_storage_for_type(ctx: &ExecutableLoweringContext, ty: &NyarType, js_glue_utf8_as_anyref: bool) -> MirStorageKind {
    let _ = js_glue_utf8_as_anyref;
    if is_js_glue_host_string_type(ty) {
        return StorageKind::Value;
    }
    // Tagged unions are represented by GC structs on the WASM backend. Keep
    // their SSA values in anyref locals even when the nominal layout metadata
    // still reports a value-oriented payload representation; otherwise a
    // variant return can later be ref.cast from an i32 slot and trap with
    // `illegal cast`.
    if matches!(ty, NyarType::Union(_)) {
        return StorageKind::Reference;
    }
    ctx.storage_for_type(ty)
}

/// 对象槽是否走 gc struct：**只读注册表**，不按类型形状特判。
///
/// 宿主字符串不得在此反向污染——一旦值携带已登记 gc struct 的 layout，
/// 构造/字段读取点就按 gc struct 发射；若 ABI 槽仍判成 i32，运行时必
/// `illegal cast`。注册条件（含 `js_glue_utf8_as_anyref` 的字段判定）已由
/// `register_gc_struct_types` 冻结，这里消费同一事实，不重新推导。
pub(super) fn type_uses_gc_struct_param(
    ctx: &ExecutableLoweringContext,
    layout_id: Option<LayoutId>,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
) -> bool {
    let Some(layout_id) = layout_id
    else {
        return false;
    };
    let Some(layout) = ctx.layout_by_id(layout_id)
    else {
        return false;
    };
    // 此处若按 `layout.storage` 重新推导，会与
    // `WasmMirLowerer::layout_uses_gc_struct` 分叉。
    gc_struct_type_indices.contains_key(&layout.id)
}

pub(super) fn wasm_param_value_type_for(
    ctx: &ExecutableLoweringContext,
    ty: &NyarType,
    layout_id: Option<LayoutId>,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<u8> {
    // 注册表是对象槽的唯一权威：值携带已登记 gc struct 的 layout 时，槽必须是
    // anyref，与构造/字段读取点消费同一冻结事实。宿主字符串特判不得反向污染此处。
    if type_uses_gc_struct_param(ctx, layout_id, gc_struct_type_indices) {
        return Ok(WASM_GC_ANYREF);
    }
    // 未携带 `layout_id`（如 ABI 形参与合成 lambda 的环境槽）但类型本身是已登记
    // gc struct 的名义实例时，仍必须按注册表返回 anyref。闭包环境 layout 由
    // `LoadFunctionRef` 的 environment 操作数强制登记（见 `type_registry`），
    // 否则跨间接调用的环境槽会被降成 i32，与构造点分叉。
    if let NyarType::Nominal { layout, .. } = ty {
        if gc_struct_type_indices.contains_key(layout) {
            return Ok(WASM_GC_ANYREF);
        }
    }
    // Node / JS-glue 宿主字符串在**未携带注册 gc struct layout** 时必须是 i32 句柄
    // （`cli_get_*` / `const_utf8`）；否则 call 实参会被 coerce 成 `ref.null`，丢掉句柄。
    if is_js_glue_host_string_type(ty) {
        return Ok(VALTYPE_I32);
    }
    wasm_js_glue_abi_value_type(ctx, ty, js_glue_utf8_as_anyref)
}

/// `WasmJsGlue` ABI 位置的载体：`utf16` 与 `utf8` 共用 i32 宿主句柄。
///
/// 边界准入已由 `validate_text_encoding_projection` 按
/// `declared_text_projection` 判定（`WasiComponent` 在到达这里前被拒绝）。
/// GC 聚合字段槽仍走 `wasm_gc_field_type_byte`，继续拒绝 `utf16`。
pub(super) fn wasm_js_glue_abi_value_type(
    ctx: &ExecutableLoweringContext,
    ty: &NyarType,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<u8> {
    if matches!(ty, NyarType::Utf16) {
        return wasm_js_glue_value_carrier_valtype(ty).ok_or_else(|| {
            crate::lowering::features::physical_contract::UTF16_UNRESOLVED_TEXT_ABI.to_string()
        });
    }
    wasm_param_value_type(ctx, ty, js_glue_utf8_as_anyref)
}

pub(super) fn wasm_param_types(
    ctx: &ExecutableLoweringContext,
    mir_fn: &MirFunction,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<Vec<u8>> {
    mir_fn
        .param_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let layout_id = mir_fn
                .blocks
                .iter()
                .find(|block| block.id == mir_fn.entry)
                .and_then(|block| block.parameters.get(index))
                .and_then(|value| mir_fn.value_layouts.get(value))
                .copied();
            wasm_param_value_type_for(ctx, ty, layout_id, gc_struct_type_indices, js_glue_utf8_as_anyref)
        })
        .collect()
}

pub(super) fn wasm_gc_field_type_byte_for_glue(ty: &NyarType, js_glue_utf8_as_anyref: bool) -> crate::Result<u8> {
    // Node/WASI 宿主字符串默认是 i32 句柄（`const_utf8` / `utf8_*`）；
    // 仅当显式打开 js_glue_utf8_as_anyref 时才把 utf8/utf16 放进 anyref 字段/数组元。
    if is_js_glue_host_string_type(ty) {
        return Ok(if js_glue_utf8_as_anyref { VALTYPE_ANYREF } else { VALTYPE_I32 });
    }
    wasm_gc_field_type_byte(ty)
}

/// 函数值签名的 ABI 载体：函数值跨 `call_indirect` 时只认托管形态。
///
/// 函数值自身是闭包对象（anyref），其 code 槽指向的函数体按托管约定接收
/// `utf8`，因此 `NyarType::Function` 的可见形参/返回值必须与函数体同一张
/// 载体表；宿主 i32 句柄只允许出现在真正的导入/导出边界，由边界物化点
/// 转换。若此处仍按 `None` layout 推导成 i32，`is_less_or_equal` 等间接
/// 调用点会与闭包体的 anyref 形参分叉。
pub(super) fn wasm_function_abi_value_type_for(
    ctx: &ExecutableLoweringContext,
    ty: &NyarType,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<u8> {
    if is_js_glue_host_string_type(ty) {
        return Ok(WASM_GC_ANYREF);
    }
    wasm_param_value_type_for(ctx, ty, None, gc_struct_type_indices, js_glue_utf8_as_anyref)
}

/// wasm/wasi 强制 GC：heap `[T]` / fixed array 在 layout 里常为 Value，
/// 但 lowering 必须用 anyref + `array.*`，不能分配 i32 槽后 `ref.cast`。
pub(super) fn type_is_wasm_gc_heap_reference(ty: &NyarType) -> bool {
    matches!(ty, NyarType::Array(_) | NyarType::FixedArray { .. })
}

/// 该类型在 wasm 线性内存里是否是**带字段的内联聚合**。
///
/// 只有带字段的内联聚合的 SSA 槽才持有线性内存地址，`memory.copy` 才是正确的
/// 赋值/字段写入方式。零字段值布局（`[primitive(...)] structure usize { }`、
/// 空结构）与所有标量载体（整数/浮点/bool/unit/字符串句柄）的槽直接持有载体
/// 本身；对这些形状使用 `memory.copy` 会把载体数值当地址解引用，产生静默错误结果。
pub(super) fn type_is_inline_aggregate(ctx: &ExecutableLoweringContext, ty: &NyarType) -> bool {
    match ty {
        NyarType::Tuple(_) | NyarType::FixedArray { .. } => true,
        NyarType::Nominal { layout, .. } => ctx.layout_by_id(*layout).is_some_and(|layout| !layout.fields.is_empty()),
        _ => false,
    }
}

/// Maps a MIR parameter type to its WASM value-type byte.
pub(super) fn wasm_param_value_type(
    ctx: &ExecutableLoweringContext,
    ty: &NyarType,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<u8> {
    let _ = js_glue_utf8_as_anyref;
    if let Some(reason) = crate::lowering::features::physical_contract::unresolved_text_abi_reason(ty) {
        // 载体事实只有一个来源：`physical_contract::declared_text_projection`。
        // `utf16` 尚未在该边界落地载体，禁止借用 `utf8` 的 i32 宿主句柄。
        return Err(reason.to_string());
    }
    if is_js_glue_host_string_type(ty) {
        return Ok(VALTYPE_I32);
    }
    if type_is_wasm_gc_heap_reference(ty) {
        return Ok(WASM_GC_ANYREF);
    }
    // Aggregates carrying references are represented by wasm-gc structs.
    // Keep their ABI as anyref even when stale nominal layout metadata says
    // Value; otherwise callers ref.cast an i32 return and trap.
    if matches!(ty, NyarType::Union(_)) {
        return Ok(WASM_GC_ANYREF);
    }
    // 函数值 = 闭包对象（gc struct `[code: i32, env: anyref]`），见合同 §5.3。
    // 无捕获路径同样使用该载体（env 为 `ref.null anyref`），因此 SSA 槽一律 anyref；
    // 不再把 funcref table 槽位下标当作函数值本身。
    if matches!(ty, NyarType::Function(_)) {
        return Ok(WASM_GC_ANYREF);
    }
    // 引用类型参数用 anyref (VALTYPE_ANYREF) 传递;值类型参数仍用 i32 地址。
    if ctx.storage_for_type(ty) == StorageKind::Reference {
        return Ok(WASM_GC_ANYREF);
    }
    Ok(wasm_scalar_value_type(ty)?)
}

/// 标量 `NyarType` 的 wasm 载体映射。
///
/// 只有 `f64` 有冻结的 `VALTYPE_F64` 载体；`f32` 没有冻结载体，必须失败闭合，
/// 不得静默拓宽到 64 位——那会让「32 位类型」实际承载 64 位载荷，位宽与 NaN
/// payload 宽度都对外失真。全量 f32（独立 `VALTYPE_F32`、精度与舍入合同）另立任务。
fn wasm_scalar_value_type(ty: &NyarType) -> crate::Result<u8> {
    match ty {
        NyarType::Float64 => Ok(VALTYPE_F64),
        NyarType::Float32 => Err(
            "WASM 参数/返回值没有 `f32` 的冻结载体：`f32` 不得静默拓宽到 64 位 `VALTYPE_F64` 载体\
             （32 位承诺与 64 位载荷矛盾）；需要独立的 `VALTYPE_F32` 载体与精度/舍入合同"
                .to_string(),
        ),
        NyarType::Integer64 { .. } | NyarType::Integer128 { .. } => Ok(VALTYPE_I64),
        _ => Ok(VALTYPE_I32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归：`f32` 不得映射到 64 位载体，必须给出点名 `f32` 的结构化失败。
    #[test]
    fn f32_has_no_frozen_wasm_carrier() {
        let error = wasm_scalar_value_type(&NyarType::Float32).expect_err("`f32` 必须 fail-closed");
        assert!(error.contains("f32"), "{error}");
        assert!(error.contains("VALTYPE_F32"), "{error}");
        assert_eq!(wasm_scalar_value_type(&NyarType::Float64).expect("`f64` 载体未变"), VALTYPE_F64);
    }
}

/// 计算 MIR 函数返回值对应的 WASM 值类型字节。
pub(super) fn wasm_return_value_type(
    ctx: &ExecutableLoweringContext,
    mir_fn: &MirFunction,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<Option<u8>> {
    match &mir_fn.return_type {
        NyarType::Unit | NyarType::Bottom => Ok(None),
        other => Ok(Some(wasm_param_value_type_for(ctx, other, mir_fn.return_layout, gc_struct_type_indices, js_glue_utf8_as_anyref)?)),
    }
}

pub(super) fn layout_needs_gc_struct(ctx: &ExecutableLoweringContext, layout: &AggregateLayout) -> bool {
    let has_ref_fields = layout.fields.iter().any(|field| ctx.storage_for_type(&field.ty) == StorageKind::Reference);
    layout.storage == StorageKind::Reference || has_ref_fields
}
