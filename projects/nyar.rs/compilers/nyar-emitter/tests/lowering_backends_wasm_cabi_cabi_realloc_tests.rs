use acorn_wasm::{WasmMiscOpcode, WasmOpcode};
use nyar_emitter::lowering::backends::wasm::cabi::{
    CABI_HEAP_DEFAULT_BASE, LINEAR_HEAP_MIN_BASE, align_up_u32, cabi_heap_base_after_data, cabi_heap_global_section,
    memory_min_pages_for_heap_base, wasm_cabi_realloc_bump_body,
};

#[test]
fn bump_body_is_not_null_stub() {
    let body = wasm_cabi_realloc_bump_body();
    assert!(body.contains(&WasmOpcode::GlobalGet.as_u8()), "global.get");
    assert!(body.windows(2).any(|w| w == [WasmOpcode::MemorySize.as_u8(), 0]), "memory.size");
    assert!(body.windows(2).any(|w| w == [WasmOpcode::PrefixMisc.as_u8(), WasmMiscOpcode::MemoryCopy.as_u8()]), "memory.copy");
}

#[test]
fn heap_global_initializes_past_static_data() {
    assert_eq!(cabi_heap_base_after_data(0), LINEAR_HEAP_MIN_BASE);
    assert_eq!(cabi_heap_base_after_data(4097), 4104);
    assert_eq!(align_up_u32(5, 4), 8);
    let section = cabi_heap_global_section(CABI_HEAP_DEFAULT_BASE);
    assert_eq!(section.id, 6);
}

#[test]
fn memory_pages_cover_heap_base_past_one_page() {
    assert_eq!(memory_min_pages_for_heap_base(LINEAR_HEAP_MIN_BASE), 1);
    assert_eq!(memory_min_pages_for_heap_base(65536), 1);
    assert_eq!(memory_min_pages_for_heap_base(65537), 2);
    assert_eq!(memory_min_pages_for_heap_base(113864), 2);
}

#[test]
fn bump_body_checks_overflow_and_zero_fills() {
    let body = wasm_cabi_realloc_bump_body();
    assert!(body.windows(2).any(|w| w == [WasmOpcode::I32LtU.as_u8(), WasmOpcode::If.as_u8()]));
    assert!(body.windows(2).any(|w| w == [WasmOpcode::PrefixMisc.as_u8(), WasmMiscOpcode::MemoryFill.as_u8()]));
}
