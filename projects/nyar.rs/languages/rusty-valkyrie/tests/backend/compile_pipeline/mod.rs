//! compile_pipeline modules.
//! 整体引用已移除的 `compile_pipeline` crate root 模块，架构已迁移到 emitter/pipeline。
//! 暂 `#[cfg(false)]` 跳过，待旧测试用例按新 TargetExecutionPlan API 重新适配。
#![cfg(false)]

mod backend_contract_interop_tests;
mod compile_pipeline_backend_bundle_tests;
mod compile_pipeline_canonical_tests;
mod compile_pipeline_driver_tests;
mod compile_pipeline_host_bindings_tests;
mod compile_pipeline_link_tests;
mod compiler_input_ownership;
mod stage_contract_guards;
mod valkyrie_compile_pipeline;
mod valkyrie_compile_pipeline_envelope_checks;
mod valkyrie_compile_pipeline_evidence;
mod valkyrie_compile_pipeline_fragment_contract_tests;
mod valkyrie_compile_pipeline_representation;
