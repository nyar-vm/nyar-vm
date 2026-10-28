//! legion integration tests.

// TODO: mod cache; mod cmds; mod forward; mod unity_export; — 私有实现测试，待公开 API 后再在 main 里 `mod` 接线
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
mod legion_manifest;
mod legion_pack_bootstrap_aliases;
mod legion_planner;
mod legion_production_lane_build_contract;
mod legion_production_lane_contract;
mod legion_valkyrie_v_leetcode_plan;
mod legion_vcc_data_consumer_inventory;
mod package_module_boundary;
mod support;
mod workspace;
