//! hir modules.

mod call_evidence_contract;
mod call_ssa_result_binding;
mod hir_collection_literal_tests;
mod hir_control_flow_validation_identity_tests;
mod hir_function_reference_tests;
mod hir_generic_callable_instantiation_tests;
mod hir_impl_method_where_obligation_tests;
mod hir_imported_namespace_call_tests;
mod hir_lowering_host_binding_pipeline_tests;
mod hir_lowering_range_call_repro_tests;
#[cfg(false)] // types::hir::compiler 路径已不存在（架构迁移）
mod hir_lowering_source_group_tests;
mod hir_lowering_template_expand_tests;
mod hir_lowering_vx_tests;
mod hir_overload_identity_tests;
#[cfg(false)] // AstToHir 已移除、类型签名变化（架构迁移）
mod hir_overload_singleton_tests;
mod hir_trait_dispatch_evidence_tests;
mod hir_trait_system_impl_identity_tests;
mod hir_type_lowering_tests;
mod valkyrie_frontend_contract_nyar_type;
mod valkyrie_frontend_error;
mod valkyrie_frontend_naming;
mod valkyrie_frontend_staging_tests;
mod valkyrie_hir_call_binding;
mod valkyrie_hir_construct_binding;
mod valkyrie_hir_lowering_expr_lowering_assignment_lowering_tests;
mod valkyrie_hir_lowering_sum_discriminator_tests;
mod valkyrie_hir_lowering_template_const;
mod valkyrie_hir_lowering_where_contract_tests;
mod valkyrie_hir_nominal_nominal_contract_tests;
mod valkyrie_hir_type_relation_evidence_tests;
mod valkyrie_hir_type_relation_identity_tests;
#[cfg(false)] // frontend::ast / crate::compiler 用法过时
mod valkyrie_parser;
#[cfg(false)] // `type_checker` crate root 模块不存在（架构迁移）
mod valkyrie_type_checker_control_flow_validation;
