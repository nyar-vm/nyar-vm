//! `legion test` 内置 OVM 执行链端到端测试。
//!
//! 锁定事实：ovm（产品别名 `legion`）target 的测试入口由 legion 内置的 odin-vm
//! 解释器进程内执行；不存在独立 `ovm` 指令，也不解析外部 `odin-vm` 二进制路径，
//! 外部 runner 覆盖对 OVM 家族一律无效。

use rusty_valkyrie::legion::{
    cmds::test_engine::{compile_external_test_session, run_external_test_in_session},
    workspace::planner::WorkspaceResolver,
};

use crate::support::create_smoke_project_with_manifest;

fn write_test_suite(project_dir: &std::path::Path) {
    let test_dir = project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(
        test_dir.join("suite.v"),
        r#"[test]
micro add_two() -> i64 {
    return 2
}

[test]
micro always_pass() -> i64 {
    return 0
}
"#,
    )
    .unwrap();
}

#[test]
fn ovm_test_entries_execute_in_builtin_runtime_ignoring_external_runner() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-ovm-builtin",
        r#"{
    name: "ovm_builtin_app",
    auto_link: { core: false, std: false },
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);

    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session = compile_external_test_session(&workspace, &fixture.project_dir, "legion").expect("ovm 测试构建必须产出执行会话");

    // 指向不存在二进制的 runner 覆盖必须被忽略：若执行链仍 spawn 外部命令，
    // 这里会以 spawn 失败暴露为测试失败。
    let bogus_runner = ["ovm=legion-test-not-a-real-binary".to_string()];
    for entry in ["add_two", "always_pass"] {
        let outcome = run_external_test_in_session(&workspace, &session, entry, &bogus_runner, false);
        assert!(outcome.success, "内置 OVM 执行 `{entry}` 失败: {:?}", outcome.error);
        assert!(!outcome.is_compile_error, "`{entry}` 不应是编译期失败: {:?}", outcome.error);
        assert_eq!(outcome.error, None);
    }
}

#[test]
fn ovm_test_session_fail_closed_on_unknown_entry() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-ovm-failclosed",
        r#"{
    name: "ovm_failclosed_app",
    auto_link: { core: false, std: false },
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);

    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session = compile_external_test_session(&workspace, &fixture.project_dir, "legion").expect("ovm 测试构建必须产出执行会话");

    let outcome = run_external_test_in_session(&workspace, &session, "not_a_test_entry", &[], false);
    assert!(!outcome.success, "不存在的测试入口不得静默通过");
    assert!(outcome.is_compile_error, "缺失执行合同必须按编译期失败处理: {:?}", outcome.error);
}
