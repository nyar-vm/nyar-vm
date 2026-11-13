//! Mandatory wasm-gc type builders for language reference semantics on wasm/wasi.
//!
//! f32 载体合同（冻结，不可协商）：
//!
//! 1. **载体身份**：`f32` 的 valtype 是独立的 [`VALTYPE_F32`]（`0x7D`），
//!    局部/参数/返回/字段/元素槽都按真实 4 字节存放。禁止把 `f32` 装进
//!    `VALTYPE_F64` 槽（含 JS glue、GC boxing、签名驱动的搬运）——那会让
//!    「32 位类型」实际承诺 64 位载荷，位宽与 NaN payload 宽度对外失真。
//! 2. **转换规则**：`f32 → f64` 只能走 `f64.promote_f32`（精确无损）；
//!    `f64 → f32` 只能走 `f32.demote_f64`（单指令 round-to-nearest-even，
//!    溢出到 `inf`、下溢到 0 或次正规）。不存在隐式拓宽，也不允许先存别的槽再搬。
//! 3. **运算与位模式**：f32 算术/比较在单精度上做，不提升到 f64 再降回；
//!    `i32.reinterpret_f32` / `f32.reinterpret_i32` 是 32 位、`i32` 载体、
//!    23 位 NaN payload 与 `-0.0`/次正规保位。
//!
//! `f32` valtype 的身份（`0x7D`）由 `acorn-wasm` 拥有；本模块只重导出，
//! 其余模块必须引用它，不得各自写字面量。
use acorn_wasm::{VALTYPE_ANYREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64, encode_arraytype_raw, encode_structtype_raw};
use nyar::NyarType;

/// Abbreviated `anyref` valtype byte (`acorn-wasm` `VALTYPE_ANYREF`).
pub(crate) const WASM_GC_ANYREF: u8 = VALTYPE_ANYREF;

/// `f32` 的 valtype 字节（Wasm 标准 `0x7D`，由 `acorn-wasm` 定义）。
pub(crate) use acorn_wasm::VALTYPE_F32;

/// WASM-GC structtype 类型段条目。
pub(crate) fn wasm_gc_struct_type(field_types: &[u8]) -> Vec<u8> {
    encode_structtype_raw(field_types)
}

/// WASM-GC arraytype 类型段条目（GC MVP）。
pub(crate) fn wasm_gc_array_type(element_type: u8) -> Vec<u8> {
    encode_arraytype_raw(element_type)
}

/// GC 聚合字段槽/数组元素的载体映射。
///
/// `None` 表示该语义类型没有已冻结的 wasm 载体；调用点必须结构化失败并点名
/// 类型，禁止回落到 i32 假载体。新增 `NyarType` 分支时只改这里。
///
/// 边界语义：本表只服务 GC 聚合槽。`WasmJsGlue` 的 ABI 位置另有
/// [`wasm_js_glue_value_carrier_valtype`]；`utf16` 在这里保持拒绝，避免宿主句柄
/// 混进聚合字段。
pub(crate) fn wasm_gc_value_carrier_valtype(ty: &NyarType) -> Option<u8> {
    Some(match ty {
        // `utf16` 没有已冻结的 wasm 聚合槽载体：它不得与 `utf8` 共用 i32 宿主句柄
        // （句柄不携带编码身份），也不得默认落线性内存 i32。只有显式声明的
        // `WasmJsGlue` ABI 位置才由 `wasm_js_glue_value_carrier_valtype` 放行。
        NyarType::Utf16 => return None,
        // f32 有独立载体：字段/元素槽按真实 4 字节存放，不与 f64 共用。
        NyarType::Float32 => VALTYPE_F32,
        NyarType::Float64 => VALTYPE_F64,
        NyarType::Integer64 { .. } | NyarType::Integer128 { .. } => VALTYPE_I64,
        // 引用语义族统一 anyref：`Apply` 是已代入的泛型实例级布局，`Function` 是
        // 闭包对象（code + env），两者若落 i32 会与构造/调用点的 anyref 槽分叉。
        NyarType::Named(_)
        | NyarType::Nominal { .. }
        | NyarType::Apply(_, _)
        | NyarType::Function(_)
        | NyarType::Array(_)
        | NyarType::TraitObject(_)
        | NyarType::Nullable(_)
        | NyarType::Union(_) => WASM_GC_ANYREF,
        // 标量与会话句柄族：宿主字符串的 i32 句柄、布尔、窄整数、字符、单元。
        NyarType::Bottom
        | NyarType::Unit
        | NyarType::Boolean
        | NyarType::Integer8 { .. }
        | NyarType::Integer16 { .. }
        | NyarType::Integer32 { .. }
        | NyarType::Character
        | NyarType::Utf8 => VALTYPE_I32,
        // 带字段的内联聚合（元组/定长数组）在字段/元素槽里持有线性内存地址。
        NyarType::Tuple(_) | NyarType::FixedArray { .. } => VALTYPE_I32,
    })
}

/// `WasmJsGlue` 边界上的值载体映射：`utf8` 与 `utf16` 共用 i32 宿主句柄。
///
/// 只有真正位于 JS-glue ABI 位置（形参、返回值、宿主导入签名）的 `utf16` 才消费
/// 本表；GC 聚合字段槽继续走 [`wasm_gc_value_carrier_valtype`] 并保持拒绝。
/// `WasiComponent` 的 `utf16` 没有冻结载体，仍由
/// `physical_contract::declared_text_projection` 在 verifier 处 fail-closed。
pub(crate) fn wasm_js_glue_value_carrier_valtype(ty: &NyarType) -> Option<u8> {
    match ty {
        NyarType::Utf8 | NyarType::Utf16 => Some(VALTYPE_I32),
        other => wasm_gc_value_carrier_valtype(other),
    }
}

/// 将 NyarType 映射到 wasm-gc 字段/元素值类型字节。
///
/// 缺失分支是计划事实缺失：返回结构化错误并点名类型，不退回 i32。
pub(crate) fn wasm_gc_field_type_byte(ty: &NyarType) -> crate::Result<u8> {
    wasm_gc_value_carrier_valtype(ty).ok_or_else(|| match ty {
        NyarType::Utf16 => {
            "WASM GC 字段/元素槽没有 `utf16` 的冻结载体：`utf16` 不得借用 `utf8` 的 i32 宿主句柄（句柄不携带编码身份），\
             也不能默认落线性内存 i32；需要显式 UTF-16 文本 ABI 契约为该槽提供载体"
                .to_string()
        }
        NyarType::Float32 => "WASM GC 字段/元素槽的 `f32` 载体映射缺失：必须返回 `VALTYPE_F32`（`0x7D`），\
            禁止拓宽到 `VALTYPE_F64` 或借用 i32 假载体"
            .to_string(),
        other => format!("WASM GC 字段/元素载体映射缺少类型分支: {other:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_keeps_host_handle_carrier_but_utf16_has_no_frozen_carrier() {
        assert_eq!(
            wasm_gc_value_carrier_valtype(&NyarType::Utf8),
            Some(VALTYPE_I32)
        );
        assert_eq!(wasm_gc_value_carrier_valtype(&NyarType::Utf16), None);

        let error = wasm_gc_field_type_byte(&NyarType::Utf16)
            .expect_err("`utf16` 字段/元素槽必须 fail-closed");
        assert!(error.contains("utf16"), "{error}");
        assert!(!error.contains("缺少类型分支"), "{error}");
        assert_eq!(
            wasm_gc_field_type_byte(&NyarType::Utf8).expect("`utf8` 载体未变"),
            VALTYPE_I32
        );
    }

    #[test]
    fn f32_has_independent_gc_carrier() {
        assert_eq!(wasm_gc_value_carrier_valtype(&NyarType::Float32), Some(VALTYPE_F32));
        assert_eq!(wasm_js_glue_value_carrier_valtype(&NyarType::Float32), Some(VALTYPE_F32));
        assert_eq!(wasm_gc_value_carrier_valtype(&NyarType::Float64), Some(VALTYPE_F64));
        assert_ne!(
            wasm_gc_value_carrier_valtype(&NyarType::Float32),
            wasm_gc_value_carrier_valtype(&NyarType::Float64),
            "`f32` 不得复用 `f64` 槽"
        );
        assert_ne!(wasm_gc_value_carrier_valtype(&NyarType::Float32), Some(VALTYPE_I32), "`f32` 不得借 i32 假载体");

        assert_eq!(wasm_gc_field_type_byte(&NyarType::Float32).expect("`f32` 字段/元素槽必须有载体"), 0x7D);
        assert_eq!(VALTYPE_F32, 0x7D, "`f32` valtype 必须使用 Wasm 标准字节");
    }

    #[test]
    fn js_glue_boundary_admits_utf16_without_touching_gc_field_slots() {
        assert_eq!(wasm_js_glue_value_carrier_valtype(&NyarType::Utf16), Some(VALTYPE_I32));
        assert_eq!(wasm_js_glue_value_carrier_valtype(&NyarType::Utf8), Some(VALTYPE_I32));
        // 边界入口只是委托：非文本类型仍按 GC 聚合表。
        assert_eq!(wasm_js_glue_value_carrier_valtype(&NyarType::Boolean), Some(VALTYPE_I32));
    }
}
