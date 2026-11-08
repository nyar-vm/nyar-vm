//! legion integration tests.

// 未接线的旧私有实现测试已移出 `tests/`，见 crate 根 `pending-tests/readme.md`。
mod cache;
mod cmds;
#[path = "../support/primitives.rs"]
pub(crate) mod primitives;
mod associated_projection_param_slots_e2e;
mod forward;
mod generic_struct_construct_context_e2e;
#[path = "../frontend/hir/hir_lowering_host_binding_pipeline_tests.rs"]
mod hir_lowering_host_binding_pipeline_tests;
mod legion_api_plan;
mod legion_artifact_formats;
mod legion_cmds_bench;
mod legion_cmds_bootstrap;
mod legion_cmds_build;
mod legion_cmds_clean_lint;
mod legion_cmds_cov;
mod legion_cmds_report;
mod legion_cmds_spy_clr;
mod legion_cmds_spy_jvm;
mod legion_cmds_spy_pe_parser;
mod legion_cmds_spy_wasm;
mod legion_cmds_test;
mod legion_external_runner_fail_closed;
mod legion_manifest;
mod legion_library_surface_contract;
mod legion_ovm_closure_p2_e2e;
mod legion_pack_bootstrap_aliases;
mod legion_planner;
mod legion_production_lane_build_contract;
mod legion_production_lane_contract;
mod legion_test_discoverability;
mod legion_test_ovm_builtin_e2e;
mod legion_test_target_resolution;
mod legion_valkyrie_v_leetcode_plan;
mod legion_valkyrie_v_std_closure;
mod legion_vcc_data_consumer_inventory;
mod legion_wasm_closure_p4_e2e;
mod package_module_boundary;
mod support;
mod unity_export;
mod variant_auto_constructor_option_e2e;
mod workspace;
