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
pub(super) use acorn_wasm::{BLOCKTYPE_EMPTY, VALTYPE_ANYREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64, WasmOpcode};

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

/// `f32` 的常量与 memarg 编码由 `acorn-wasm` 拥有；emitter 不再保留本地发射点。
pub(super) use acorn_wasm::{encode_f32_const, encode_f32_load};

pub(super) use super::super::executable::ExecutableLoweringContext;
pub(super) use nyar_types::{LayoutId, NominalInstanceId, VariantId};

// 从 crate 顶层模块 re-export
pub(super) use crate::{
    contracts::{ArrayInitialization, instruction_primary_result},
    target_operator::{TargetOperatorBinding, WasmGcArrayOperation, WasmGcArrayOperatorPlan},
};

// 从 wasm gc 模块 re-export
pub(super) use super::gc::WASM_GC_ANYREF;

// 从子模块 re-export（供兄弟子模块通过 super::* 消费）
pub(super) use lir::{WasmBodyInstruction, encode_wasm_body};
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
mod lir;
mod lowerer;
pub(crate) mod numeric;
mod operands;
mod representation;
mod strings;
mod type_registry;

// ── Facade pub(crate) entrances (wasm 后端唯一对外入口) ──────────────────

pub(crate) use entry::{augment_wasm_with_value_aggregate_metadata, lower_fragment_mir_to_wasm_module, lower_fragment_mir_to_wasm_module_for};

/// Wasm 物理导出 ABI 的私有派生点。
///
/// 公共 [`crate::TargetExecutionPlan`] 只携带逻辑导出身份与签名，构造时不再
/// 为任何目标预先冻结物理载体。Wasm lane 在需要导出 ABI 时按函数签名现场冻结；
/// 缓存存在时只复用本 lane 已冻结的事实。其它 lane 不受 Wasm 载体资格约束。
pub(super) fn wasm_library_export_abi(
    plan: &crate::TargetExecutionPlan,
    instance: &nyar_types::ItemInstanceId,
) -> crate::Result<crate::LibraryExportAbi> {
    if let Some(abi) = plan.library_export_abi(instance) {
        return Ok(abi.clone());
    }
    let view = plan.get_function(instance).ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体，无法冻结 Wasm ABI 载体合同"))?;
    crate::target_execution_plan::freeze_library_export_abi(&view.function)
}
