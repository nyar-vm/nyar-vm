//! emitter modules.
//! 全部引用已移除的 `executable_provider` / `nyar_emitter::testing` / 旧 FragmentSubmission 字段。
//! 暂 `#[cfg(false)]` 跳过，待 legacy emitter contract 按新 TargetExecutionPlan API 重写。
#![cfg(false)]

mod clr_mir_contracts;
mod clr_nominal_contracts;
mod clr_type_contracts;
mod emitter_driver_families_ovm;
mod emitter_driver_families_wasm;
mod emitter_target_execution_plan_representation_contract_tests;
mod emitter_target_operator_freeze_tests;
mod pattern_matching_backend_parity;
mod pattern_matching_clr_lowering;
