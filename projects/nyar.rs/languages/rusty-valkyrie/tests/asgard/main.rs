//! asgard integration tests.

#[path = "../support/primitives.rs"]
pub(crate) mod primitives;

mod asgard_asgard_ui_golden;
mod asgard_codegen;
mod asgard_platform_contract;
mod asgard_render_ir_canonical;
mod asgard_style_collector;
mod awsl;
mod awsl_extension_source_of_truth;
mod boot_lifecycle;
mod chunk_config_matrix;
mod chunk_planning;
mod cli_app_dev;
mod cli_dev_swap;
mod cli_static;
mod codegen;
mod config_discovery;
mod config_model;
mod config_probe;
mod css_chunks;
mod deploy;
mod glue_contract;
mod host;
mod host_backend;
mod host_validation;
mod interactive_artifact;
mod layout_contract;
mod list_count_abi;
mod list_each_shape;
mod package;
mod pipeline_discover;
mod platform_contract;
mod product_page;
mod primitive_identity_e2e;
mod report_text_literal;
mod scalar_bootstrap;
mod script_binding_inline;
mod serve;
mod serve_static_build;
mod server_runtime;
mod ssg;
mod ssg_ovm;
mod ssg_style_contract;
mod static_artifact_tree;
mod static_site;
mod support;
mod symbols_tests;
mod tailwind;

// 未接线的旧私有实现测试已移出 `tests/`，见 crate 根 `pending-tests/readme.md`。
