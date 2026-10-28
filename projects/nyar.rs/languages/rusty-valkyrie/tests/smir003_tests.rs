//! Standalone SMIR003 test harness.
//! 只引入 contract-gate ignore 标记涉及的 MIR / backend 测试。

#[path = "frontend/mir/valkyrie_mir_ssa_singleton_tests.rs"]
mod valkyrie_mir_ssa_singleton_tests;

#[path = "frontend/mir/valkyrie_mir_ssa_field_contract_tests.rs"]
mod valkyrie_mir_ssa_field_contract_tests;

#[path = "frontend/mir/valkyrie_mir_sum.rs"]
mod valkyrie_mir_sum;

#[path = "backend/compile_pipeline/backend_contract_interop_tests.rs"]
mod backend_contract_interop_tests;
