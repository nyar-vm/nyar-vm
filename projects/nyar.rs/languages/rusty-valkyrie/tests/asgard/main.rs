//! asgard integration tests.

mod asgard_asgard_ui_golden;
mod asgard_codegen;
mod asgard_platform_contract;
mod asgard_render_ir_canonical;
mod asgard_style_collector;
mod awsl_extension_source_of_truth;
mod boot_lifecycle;
mod chunk_config_matrix;
mod chunk_planning;
mod cli_app_dev;
mod cli_dev_swap;
mod cli_static;
mod config_discovery;
mod config_model;
mod config_probe;
mod glue_contract;
mod pipeline_discover;
mod serve;
mod serve_static_build;
mod server_runtime;
mod ssg;
mod static_artifact_tree;
mod static_site;
mod symbols_tests;

// Private implementation tests on disk under awsl/, codegen/, etc. — wire when using public API.
// TODO: mod deploy;
// TODO: mod host; mod host_backend; mod host_validation;
// TODO: mod awsl; mod codegen; mod package; mod platform_contract; mod tailwind;
