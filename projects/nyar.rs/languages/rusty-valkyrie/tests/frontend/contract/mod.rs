//! contract modules.

mod array_length_intrinsic_declaration_tests;
mod array_push_intrinsic_declaration_tests;
#[cfg(false)] // MirFunction 旧字段、executable_provider 已移除（架构迁移）
mod case_chain_validation;
mod frontend_contract_control_flow_payload_witness_identity_tests;
#[cfg(false)] // FragmentSubmission 旧字段已移除（架构迁移）
mod generic_struct_new_contract;
#[cfg(false)] // FragmentSubmission 旧字段已移除（架构迁移）
mod singleton_contracts;
#[cfg(false)] // `assembly` 模块不存在（架构迁移）
mod valkyrie_assembly_import_contract_tests;
#[cfg(false)] // `control_flow` 模块不存在（架构迁移）
mod valkyrie_control_flow_context;
mod valkyrie_highlight;
mod valkyrie_lint;
mod valkyrie_types_hir_export_spec;
