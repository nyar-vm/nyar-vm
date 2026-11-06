//! Mandatory wasm-gc type builders for language reference semantics on wasm/wasi.
use acorn_wasm::{VALTYPE_ANYREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64, encode_arraytype_raw, encode_structtype_raw};
use nyar::NyarType;

/// Abbreviated `anyref` valtype byte (`acorn-wasm` `VALTYPE_ANYREF`).
pub(crate) const WASM_GC_ANYREF: u8 = VALTYPE_ANYREF;

/// WASM-GC structtype 类型段条目。
pub(crate) fn wasm_gc_struct_type(field_types: &[u8]) -> Vec<u8> {
    encode_structtype_raw(field_types)
}

/// WASM-GC arraytype 类型段条目（GC MVP）。
pub(crate) fn wasm_gc_array_type(element_type: u8) -> Vec<u8> {
    encode_arraytype_raw(element_type)
}

/// 单一值载体映射：字段槽、数组元素、形参与返回值都消费这一张表。
///
/// `None` 表示该语义类型没有已冻结的 wasm 载体；调用点必须结构化失败并点名
/// 类型，禁止回落到 i32 假载体。新增 `NyarType` 分支时只改这里。
pub(crate) fn wasm_value_carrier_valtype(ty: &NyarType) -> Option<u8> {
    Some(match ty {
        // `utf16` 没有已冻结的 wasm 载体：它不得与 `utf8` 共用 i32 宿主句柄（句柄
        // 不携带编码身份），也不得默认落线性内存 i32。宿主边界携带 `utf16` 必须由
        // 显式声明的文本 ABI 契约提供载体；在此之前该槽结构化失败，禁止静默放行。
        NyarType::Utf16 => return None,
        NyarType::Float64 | NyarType::Float32 => VALTYPE_F64,
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

/// 将 NyarType 映射到 wasm-gc 字段/元素值类型字节。
///
/// 缺失分支是计划事实缺失：返回结构化错误并点名类型，不退回 i32。
pub(crate) fn wasm_gc_field_type_byte(ty: &NyarType) -> crate::Result<u8> {
    wasm_value_carrier_valtype(ty).ok_or_else(|| match ty {
        NyarType::Utf16 => {
            "WASM GC 字段/元素槽没有 `utf16` 的冻结载体：`utf16` 不得借用 `utf8` 的 i32 宿主句柄（句柄不携带编码身份），\
             也不能默认落线性内存 i32；需要显式 UTF-16 文本 ABI 契约为该槽提供载体"
                .to_string()
        }
        other => format!("WASM GC 字段/元素载体映射缺少类型分支: {other:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_keeps_host_handle_carrier_but_utf16_has_no_frozen_carrier() {
        assert_eq!(
            wasm_value_carrier_valtype(&NyarType::Utf8),
            Some(VALTYPE_I32)
        );
        assert_eq!(wasm_value_carrier_valtype(&NyarType::Utf16), None);

        let error = wasm_gc_field_type_byte(&NyarType::Utf16)
            .expect_err("`utf16` 字段/元素槽必须 fail-closed");
        assert!(error.contains("utf16"), "{error}");
        assert!(!error.contains("缺少类型分支"), "{error}");
        assert_eq!(
            wasm_gc_field_type_byte(&NyarType::Utf8).expect("`utf8` 载体未变"),
            VALTYPE_I32
        );
    }
}
