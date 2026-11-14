//! rusty-valkyrie 的最小冒烟门。
//!
//! 重型管线/后端/Asgard/Legion 套件已迁往 `valkyrie.rs` 的下游 harness。
//! 本 crate 只保留最基本的“前端可解析、编译器不 panic”检查，避免测试门禁
//! 反向约束架构重构。

use rusty_valkyrie::{ValkyrieCompiler, parser::parse_source};

#[test]
fn parses_a_minimal_function() {
    let root = parse_source("micro answer(): i32 {\n    return 42\n}\n").expect("minimal source must parse");
    assert_eq!(root.items.len(), 1, "one function declaration must survive parsing");
}

#[test]
fn minimal_entry_compiles() {
    let source = "[main] micro main() { }";
    let groups = [rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", source).as_entry()];
    let program = ValkyrieCompiler::default().compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "wasm");
    assert!(program.is_ok(), "minimal entry must compile without a structured failure: {program:?}");
}

/// 端到端最小门：源码 → Valkyrie MIR → 语言 × 目标适配器 → Wasm 产物。
///
/// 该用例覆盖正式 `AdaptedFragmentInput` 接入路径，确保语义载荷归还语言后
/// 仍能真实产出后端制品，而不是只通过手工构造计划的 lowering 测试。
#[test]
fn minimal_entry_emits_wasm_artifact() {
    use nyar::ClrSuspendStrategy;
    use nyar_emitter::nyar_backend_wasi::WasmPackageKind;
    use rusty_valkyrie::compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts};

    let groups = vec![rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", "[main] micro main() { }").as_entry()];
    let input = rusty_valkyrie::compiler::hir::lowering::CompilerSourceInput::application(groups);
    let context = CompilerBuildContext::new(
        "wasm32",
        nyar::CanonicalTarget::parse("node").expect("node target"),
        ClrSuspendStrategy::from_runtime_async_flag(false),
        WasmPackageKind::Binary,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let report = compile_source_groups_to_artifacts(
        &ValkyrieCompiler::default(),
        &input,
        &context,
        dir.path(),
        "smoke",
        false,
        false,
    )
    .expect("minimal source must produce a target artifact through the adapter");
    assert!(!report.driver.artifacts.artifacts.is_empty(), "adapter path must emit at least one artifact");
}
