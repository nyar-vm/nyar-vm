//! Semantic MIR -> Wasm executable prepare/emit (**wasm-gc** required for references).
//!
//! **Not** a second MIR. Language CFG/types stay in Semantic MIR; this facade and its
//! submodules own Wasm physical encoding only.
//!
//! - [control] — CFG block dispatch helpers
//! - [representation] — unique Wasm storage / value-type rules
//! - [type_registry] — GC type-index registration
//! - [calls] — resolved calls -> Wasm call forms
//! - [imports] — host import collection, validation, index-building
//! - [strings] — WASI string-literal data section + function-type byte helpers
//! - [lowerer] — per-function WasmMirLowerer struct + full impl
//! - [entry] — two pub(crate) entrances that drive module assembly
//! - **Value aggregates** (structure / tuple / [T; N]): linear memory
//! - **Reference aggregates** (class / heap [T]): wasm-gc struct/array ops

// ── Local re-exports (子模块之间共用) ──────────────────────────────────────

// 从 acorn_wasm re-export（calls.rs / control.rs / lowerer.rs 通过 super::* 消费）
pub(super) use acorn_wasm::{
    BLOCKTYPE_EMPTY, VALTYPE_ANYREF, VALTYPE_EXTERNREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64, VALTYPE_REF_NULL, WasmExternalKind, WasmOpcode,
    encode_array_copy, encode_array_get, encode_array_len, encode_array_new_default, encode_array_new_fixed, encode_array_set,
    encode_block_empty, encode_br, encode_br_if, encode_call, encode_drop, encode_f64_const, encode_f64_load, encode_f64_store,
    encode_global_get, encode_global_set, encode_i32_add, encode_i32_and, encode_i32_const, encode_i32_load, encode_i32_store,
    encode_i64_const, encode_i64_load, encode_i64_store, encode_local_get, encode_local_set, encode_local_tee, encode_loop_empty,
    encode_memarg, encode_memory_copy, encode_memory_fill, encode_memory_grow, encode_memory_size, encode_ref_cast_array,
    encode_ref_cast_type_index, encode_ref_null_anyref, encode_ref_null_externref, encode_sleb128_i64, encode_struct_get,
    encode_struct_new_default, encode_struct_set, encode_unreachable,
};

// 从 crate::backend_plan_views re-export
pub(super) use crate::backend_plan_views::{
    ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableConstant as MirConstant,
    ExecutableDispatchKind as MirDispatchKind, ExecutableFunction as MirFunction, ExecutableInstruction as MirInstruction,
    ExecutableInstructionKind as MirInstructionKind, ExecutableOperand as MirOperand, ExecutableReceiverPassingKind as ReceiverPassingKind,
    ExecutableStorageKind as MirStorageKind, ExecutableStorageKind as StorageKind, ExecutableTerminator as MirTerminator,
    ExecutableValueRef as MirValueRef, NyarType,
};

/// `f32` 在 wasm 侧的唯一 valtype 常量（`0x7D`）；权威定义见 `wasm::gc::VALTYPE_F32`。
pub(super) use crate::lowering::backends::wasm::gc::VALTYPE_F32;

/// `f32.const`（opcode `0x43` + 4 字节小端位模式）。
///
/// `acorn_wasm` 未导出 f32 编码器，这里是唯一发射点。必须直接用 `f32::to_bits`
/// 写 4 字节：不得经 f64 往返，否则 NaN payload 低 29 位与次正规位模式会被改写。
pub(super) fn encode_f32_const(value: f32, code: &mut Vec<u8>) {
    code.push(0x43);
    code.extend_from_slice(&value.to_bits().to_le_bytes());
}

/// `f32.load`（opcode `0x2A`，4 字节字段，自然对齐 log2 = 2）。
pub(super) fn encode_f32_load(align_log2: u32, offset: u32, code: &mut Vec<u8>) {
    code.push(0x2A);
    crate::lowering::backends::wasm::sections::encode_uleb128(align_log2, code);
    crate::lowering::backends::wasm::sections::encode_uleb128(offset, code);
}

/// `f32.store`（opcode `0x38`，4 字节字段，自然对齐 log2 = 2）。
pub(super) fn encode_f32_store(align_log2: u32, offset: u32, code: &mut Vec<u8>) {
    code.push(0x38);
    crate::lowering::backends::wasm::sections::encode_uleb128(align_log2, code);
    crate::lowering::backends::wasm::sections::encode_uleb128(offset, code);
}

pub(super) use super::super::executable::ExecutableLoweringContext;
pub(super) use nyar_types::{LayoutId, NominalInstanceId, VariantId};

// 从 crate 顶层模块 re-export
pub(super) use crate::{
    FragmentSubmission,
    contracts::{ArrayInitialization, instruction_primary_result},
    nyar_backend_wasi::WasmBinaryModule,
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
        WasmGcArrayOperation, WasmGcArrayOperatorPlan,
    },
};

// 从 wasm gc 模块 re-export
pub(super) use super::gc::WASM_GC_ANYREF;

// 从子模块 re-export（供兄弟子模块通过 super::* 消费）
pub(super) use lowerer::{
    HostHandleToManagedUtf8Plan, ManagedUtf8ToHostHandlePlan, Utf8HostHandleMaterializePlan, WASM_GC_EXTERNREF, WasmMirLowerer,
    array_element_type,
};
pub(super) use representation::*;
pub(super) use type_registry::*;

// ── Submodules ────────────────────────────────────────────────────────────

mod abi_schema;
pub(crate) mod array;
mod calls;
mod control;
mod encode;
mod entry;
mod imports;
mod instructions;
mod intrinsics;
mod library_invoke;
mod lowerer;
pub(crate) mod numeric;
mod operands;
mod representation;
mod strings;
mod type_registry;

// ── Facade pub(crate) entrances (wasm 后端唯一对外入口) ──────────────────

pub(crate) use entry::{augment_wasm_with_value_aggregate_metadata, lower_fragment_mir_to_wasm_module, lower_fragment_mir_to_wasm_module_for};
