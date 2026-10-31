//! compile_pipeline modules.
//! 各子模块按新 `TargetExecutionPlan` / emitter pipeline 逐个体检：已适配的模块直接启用，
//! 仍引用已移除 `compile_pipeline` crate root 模块的用例单文件 `#[cfg(false)]` 继续跳过，
//! 不再用模块级 `#![cfg(false)]` 把整组（含仍有效的 backend contract 门）一起关掉。

mod backend_contract_interop_tests;
#[cfg(false)] // 旧 builder bundle 合同，待按 emitter pipeline 适配
mod compile_pipeline_backend_bundle_tests;
mod canonical_function_reference_tests;
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
