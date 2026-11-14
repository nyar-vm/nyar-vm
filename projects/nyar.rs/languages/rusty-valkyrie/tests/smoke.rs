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
