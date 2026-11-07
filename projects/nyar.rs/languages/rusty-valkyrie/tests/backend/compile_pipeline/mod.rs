//! compile_pipeline modules.
//! 各子模块按新 `TargetExecutionPlan` / emitter pipeline 逐个体检：已适配的模块直接启用，
//! 仍引用已移除 `compile_pipeline` crate root 模块的用例单文件 `#[cfg(false)]` 继续跳过，
//! 不再用模块级 `#![cfg(false)]` 把整组（含仍有效的 backend contract 门）一起关掉。

mod backend_contract_interop_tests;
mod backend_f64_bits_e2e_tests;
mod backend_f64_provider_e2e_tests;
mod backend_i64_bitwise_provider_e2e_tests;
mod backend_large_i64_literal_e2e_tests;
mod backend_match_default_binding_tests;
mod backend_ovm_block_writeback_tests;
mod backend_match_null_payload_e2e_tests;
mod backend_multifield_variant_e2e_tests;
mod backend_object_pattern_binding_tests;
mod backend_ovm_array_provider_e2e_tests;
mod backend_ovm_export_surface_tests;
mod backend_ovm_match_tail_value_tests;
mod backend_ovm_operator_freeze_tests;
mod backend_ovm_text_carrier_e2e_tests;
mod backend_ovm_where_super_trait_e2e_tests;
mod backend_single_quote_literal_e2e_tests;
mod backend_trait_witness_materialization_tests;
mod backend_unsigned_shift_provider_e2e_tests;
mod backend_wasm_array_provider_e2e_tests;
mod canonical_function_reference_tests;
#[cfg(false)] // 旧 builder bundle 合同，待按 emitter pipeline 适配
mod compile_pipeline_backend_bundle_tests;
mod compile_pipeline_canonical_tests;
#[cfg(false)] // 旧 driver 入口已移除
mod compile_pipeline_driver_tests;
#[cfg(false)] // 旧 host bindings 入口已移除
mod compile_pipeline_host_bindings_tests;
#[cfg(false)] // 旧 link 入口已移除
mod compile_pipeline_link_tests;
mod compile_pipeline_semantic_diagnostic_tests;
#[cfg(false)] // `compiler_input` 旧字段已移除
mod compiler_input_ownership;
#[cfg(false)] // 旧阶段守卫按旧 stage 枚举
mod stage_contract_guards;
#[cfg(false)] // 旧 `ValkyrieCompiler` 管线入口
mod valkyrie_compile_pipeline;
#[cfg(false)] // 旧 envelope 检查
mod valkyrie_compile_pipeline_envelope_checks;
#[cfg(false)] // 旧 evidence 检查
mod valkyrie_compile_pipeline_evidence;
mod valkyrie_compile_pipeline_fragment_contract_tests;
mod valkyrie_compile_pipeline_representation;
