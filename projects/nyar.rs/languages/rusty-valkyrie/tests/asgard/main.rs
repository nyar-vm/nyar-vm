//! asgard integration tests.

mod asgard_asgard_ui_golden;
mod asgard_codegen;
mod asgard_platform_contract;
mod asgard_render_ir_canonical;
mod asgard_style_collector;
mod cli_static;
mod config_discovery;
mod config_model;
mod pipeline_discover;
mod ssg;
mod static_site;
mod symbols_tests;

// Private implementation tests on disk under awsl/, codegen/, etc. — wire when using public API.
// TODO: mod deploy;
// TODO: mod host; mod host_backend; mod host_validation;
// TODO: mod awsl; mod codegen; mod package; mod platform_contract; mod tailwind;
