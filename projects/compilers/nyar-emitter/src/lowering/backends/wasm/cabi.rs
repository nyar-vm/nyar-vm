//! Canonical ABI linear heap: shared bump cursor for `cabi_realloc` and MIR allocations.
use crate::nyar_backend_wasi::WasmSection;
use acorn_wasm::{
    VALTYPE_I32, WasmOpcode, encode_i32_and, encode_i32_const, encode_if_empty, encode_local_get, encode_local_set, encode_local_tee,
    encode_memory_copy, encode_memory_fill, encode_memory_grow, encode_memory_size, encode_return, encode_unreachable,
};

use super::sections::{encode_uleb128, global_section_with_i32_inits};

/// 线性堆最小起点（集成测试专用）。
#[doc(hidden)]
pub const LINEAR_HEAP_MIN_BASE: i32 = 4096;
/// 默认 `cabi` 堆基址（集成测试专用）。
#[doc(hidden)]
pub const CABI_HEAP_DEFAULT_BASE: i32 = LINEAR_HEAP_MIN_BASE;
/// 承载 `cabi` 堆游标的 wasm 全局索引（集成测试专用）。
#[doc(hidden)]
pub const CABI_HEAP_GLOBAL_INDEX: u32 = 0;

/// 将数值按给定对齐向上取整（集成测试专用）。
#[doc(hidden)]
pub fn align_up_u32(value: u32, align: u32) -> u32 {
    let align = align.max(1);
    let mask = align - 1;
    (value + mask) & !mask
}

/// 由静态数据长度推导 `cabi` 堆基址（集成测试专用）。
#[doc(hidden)]
pub fn cabi_heap_base_after_data(data_len: usize) -> i32 {
    let aligned = align_up_u32(u32::try_from(data_len).unwrap_or(u32::MAX), 8);
    i32::try_from(aligned.max(LINEAR_HEAP_MIN_BASE as u32)).unwrap_or(LINEAR_HEAP_MIN_BASE)
}

/// Minimum linear-memory pages so static data + cabi heap cursor fit at instantiate.
/// `heap_base` is the first free byte; active data occupies `[0, heap_base)`.
#[doc(hidden)]
pub fn memory_min_pages_for_heap_base(heap_base: i32) -> u32 {
    let bytes = u64::from(heap_base.max(LINEAR_HEAP_MIN_BASE) as u32);
    let pages = bytes.div_ceil(65536).max(1);
    u32::try_from(pages).unwrap_or(u32::MAX)
}

/// 构造承载 `cabi` 堆基址的 wasm 全局段（集成测试专用）。
#[doc(hidden)]
pub fn cabi_heap_global_section(heap_base: i32) -> WasmSection {
    debug_assert!(heap_base >= LINEAR_HEAP_MIN_BASE, "heap base must be >= {LINEAR_HEAP_MIN_BASE}");
    global_section_with_i32_inits(&[heap_base.max(LINEAR_HEAP_MIN_BASE)])
}

/// 构造 `cabi_realloc` 的 bump 分配函数体（集成测试专用）。
#[doc(hidden)]
pub fn wasm_cabi_realloc_bump_body() -> Vec<u8> {
    let mut body = Vec::new();
    encode_uleb128(1, &mut body);
    encode_uleb128(4, &mut body);
    body.push(VALTYPE_I32);

    encode_local_get(3, &mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    encode_if_empty(&mut body);
    encode_i32_const(0, &mut body);
    encode_return(&mut body);
    WasmOpcode::End.encode(&mut body);

    encode_local_get(0, &mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    encode_local_get(3, &mut body);
    encode_local_get(1, &mut body);
    WasmOpcode::I32LeU.encode(&mut body);
    encode_i32_and(&mut body);
    encode_if_empty(&mut body);
    encode_local_get(0, &mut body);
    encode_return(&mut body);
    WasmOpcode::End.encode(&mut body);

    encode_local_get(2, &mut body);
    encode_i32_const(1, &mut body);
    WasmOpcode::I32LtU.encode(&mut body);
    encode_if_empty(&mut body);
    encode_i32_const(1, &mut body);
    encode_local_set(2, &mut body);
    WasmOpcode::End.encode(&mut body);

    WasmOpcode::GlobalGet.encode(&mut body);
    encode_uleb128(CABI_HEAP_GLOBAL_INDEX, &mut body);
    encode_local_get(2, &mut body);
    WasmOpcode::I32Add.encode(&mut body);
    encode_i32_const(1, &mut body);
    WasmOpcode::I32Sub.encode(&mut body);
    encode_local_get(2, &mut body);
    encode_i32_const(1, &mut body);
    WasmOpcode::I32Sub.encode(&mut body);
    encode_i32_const(-1, &mut body);
    WasmOpcode::I32Xor.encode(&mut body);
    encode_i32_and(&mut body);
    encode_local_set(4, &mut body);

    encode_local_get(4, &mut body);
    encode_local_get(3, &mut body);
    WasmOpcode::I32Add.encode(&mut body);
    encode_local_tee(5, &mut body);
    encode_local_get(4, &mut body);
    WasmOpcode::I32LtU.encode(&mut body);
    encode_if_empty(&mut body);
    encode_unreachable(&mut body);
    WasmOpcode::End.encode(&mut body);

    encode_local_get(5, &mut body);
    encode_i32_const(65535, &mut body);
    WasmOpcode::I32Add.encode(&mut body);
    encode_i32_const(16, &mut body);
    WasmOpcode::I32ShrU.encode(&mut body);
    encode_memory_size(&mut body);
    WasmOpcode::I32Sub.encode(&mut body);
    encode_local_tee(6, &mut body);
    encode_i32_const(0, &mut body);
    WasmOpcode::I32GtS.encode(&mut body);
    encode_if_empty(&mut body);
    encode_local_get(6, &mut body);
    encode_memory_grow(&mut body);
    encode_i32_const(-1, &mut body);
    WasmOpcode::I32Eq.encode(&mut body);
    encode_if_empty(&mut body);
    encode_unreachable(&mut body);
    WasmOpcode::End.encode(&mut body);
    WasmOpcode::End.encode(&mut body);

    encode_local_get(4, &mut body);
    encode_i32_const(0, &mut body);
    encode_local_get(3, &mut body);
    encode_memory_fill(&mut body);

    encode_local_get(0, &mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    encode_local_get(1, &mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    WasmOpcode::I32Eqz.encode(&mut body);
    encode_i32_and(&mut body);
    encode_if_empty(&mut body);
    encode_local_get(1, &mut body);
    encode_local_get(3, &mut body);
    encode_local_get(1, &mut body);
    encode_local_get(3, &mut body);
    WasmOpcode::I32LtU.encode(&mut body);
    WasmOpcode::Select.encode(&mut body);
    encode_local_set(7, &mut body);
    encode_local_get(4, &mut body);
    encode_local_get(0, &mut body);
    encode_local_get(7, &mut body);
    encode_memory_copy(&mut body);
    WasmOpcode::End.encode(&mut body);

    encode_local_get(5, &mut body);
    WasmOpcode::GlobalSet.encode(&mut body);
    encode_uleb128(CABI_HEAP_GLOBAL_INDEX, &mut body);
    encode_local_get(4, &mut body);
    WasmOpcode::End.encode(&mut body);
    body
}
