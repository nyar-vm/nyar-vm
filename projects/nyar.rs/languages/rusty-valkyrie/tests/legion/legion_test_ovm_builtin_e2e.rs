//! `legion test` 内置 OVM 执行链端到端测试。
//!
//! 锁定事实：ovm（产品别名 `legion`）target 的测试入口由 legion 内置的 odin-vm
//! 解释器进程内执行；不存在独立 `ovm` 指令，也不解析外部 `odin-vm` 二进制路径，
//! 外部 runner 覆盖对 OVM 家族一律无效。

use rusty_valkyrie::legion::{
    cmds::test_engine::{compile_external_test_session, discover_test_functions, run_external_test_in_session},
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

/// 复现用：`test/suite.v` 里连续写 8 个 `[test]`，统计执行合同条数与名字。
fn suite_with_tests(names: &[&str]) -> String {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| format!("[test]\nmicro {name}() -> i64 {{\n    return {}\n}}\n", index + 1))
        .collect::<String>()
}

fn test_contract_names(prefix: &str, target_label: &str, names: &[&str], source: &str) -> Vec<String> {
    let fixture = create_smoke_project_with_manifest(
        prefix,
        r#"{
    name: "contract_count_app",
    build: [ { target: "ovm" }, { target: "node" } ]
}
"#,
        source,
    );
    let test_dir = fixture.project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("suite.v"), suite_with_tests(names)).unwrap();
    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session =
        compile_external_test_session(&workspace, &fixture.project_dir, target_label).expect("测试构建必须产出执行会话");
    session.run_contracts.iter().map(|contract| contract.logical_entry.clone()).collect()
}

fn ovm_test_contract_names(prefix: &str, names: &[&str], source: &str) -> Vec<String> {
    test_contract_names(prefix, "legion", names, source)
}

const PLAIN_MAIN: &str = r#"micro main() -> i64 {
    return 0
}
"#;

const ANNOTATED_MAIN: &str = r#"[main]
micro main() -> i64 {
    return 0
}
"#;

#[test]
fn every_test_annotation_yields_one_run_contract() {
    let eight = ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8"];
    let names = ovm_test_contract_names("legion-test-contract-eight", &eight, PLAIN_MAIN);
    assert_eq!(names, eight.to_vec(), "8 个 `[test]` 必须产出 8 条合同");
}

#[test]
fn every_test_annotation_yields_one_run_contract_with_annotated_main() {
    let eight = ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8"];
    let names = ovm_test_contract_names("legion-test-contract-eight-main", &eight, ANNOTATED_MAIN);
    assert_eq!(names, eight.to_vec(), "带 `[main]` 时 8 个 `[test]` 仍必须产出 8 条合同");
}

#[test]
fn leading_dummy_test_does_not_change_contract_count() {
    let suite = ["dummy", "b1", "b2", "b3", "b4", "b5", "b6", "b7", "b8"];
    let names = ovm_test_contract_names("legion-test-contract-dummy", &suite, PLAIN_MAIN);
    assert_eq!(names, suite.to_vec(), "首位 dummy 不得让合同数量或顺序偏移");
}

#[test]
fn single_test_annotation_yields_one_run_contract() {
    let names = ovm_test_contract_names("legion-test-contract-one", &["only"], PLAIN_MAIN);
    assert_eq!(names, vec!["only".to_string()], "单个 `[test]` 必须产出 1 条合同");
}

#[test]
fn three_test_annotations_yield_three_run_contracts_in_order() {
    let three = ["t1", "t2", "t3"];
    let names = ovm_test_contract_names("legion-test-contract-three", &three, PLAIN_MAIN);
    assert_eq!(names, three.to_vec(), "3 个 `[test]` 必须产出 3 条合同且保持声明顺序");
}

#[test]
fn node_lane_every_test_annotation_yields_one_run_contract() {
    let eight = ["n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8"];
    let names = test_contract_names("legion-test-contract-node-eight", "node", &eight, PLAIN_MAIN);
    assert_eq!(names, eight.to_vec(), "node lane 的 8 个 `[test]` 必须各自产出一条合同");
}

#[test]
fn two_namespaced_test_files_keep_each_first_test_contract() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-contract-two-files",
        r#"{
    name: "contract_two_files_app",
    build: [ { target: "ovm" } ]
}
"#,
        PLAIN_MAIN,
    );
    let test_dir = fixture.project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(
        test_dir.join("alpha.v"),
        format!("namespace suite.alpha;\n\n# 首测说明\n\n{}", suite_with_tests(&["a1", "a2", "a3", "a4"])),
    )
    .unwrap();
    std::fs::write(
        test_dir.join("beta.v"),
        format!("namespace suite.beta;\n\n# 首测说明\n\n{}", suite_with_tests(&["b1", "b2", "b3", "b4"])),
    )
    .unwrap();
    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session = compile_external_test_session(&workspace, &fixture.project_dir, "legion").expect("测试构建必须产出执行会话");
    let names = session.run_contracts.iter().map(|contract| contract.logical_entry.clone()).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["a1", "a2", "a3", "a4", "b1", "b2", "b3", "b4"],
        "每个测试文件的首个 `[test]` 都必须保留自己的合同: {names:?}"
    );
}

#[test]
fn bom_prefixed_suite_file_keeps_every_run_contract() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-contract-bom",
        r#"{
    name: "contract_bom_app",
    build: [ { target: "ovm" } ]
}
"#,
        PLAIN_MAIN,
    );
    let test_dir = fixture.project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    let eight = ["c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8"];
    std::fs::write(test_dir.join("suite.v"), format!("\u{feff}{}", suite_with_tests(&eight))).unwrap();
    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session = compile_external_test_session(&workspace, &fixture.project_dir, "legion").expect("测试构建必须产出执行会话");
    let names = session.run_contracts.iter().map(|contract| contract.logical_entry.clone()).collect::<Vec<_>>();
    assert_eq!(names, eight.to_vec(), "UTF-8 BOM 不得吞掉首个 `[test]` 的执行合同: {names:?}");
}

fn discovered_test_names(path: &std::path::Path) -> Vec<String> {
    discover_test_functions(&[path]).into_iter().filter(|function| function.is_test).map(|function| function.name).collect()
}

#[test]
fn discovered_tests_match_run_contracts_for_plain_suite() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-discover-plain",
        r#"{
    name: "discover_plain_app",
    build: [ { target: "ovm" } ]
}
"#,
        PLAIN_MAIN,
    );
    let test_dir = fixture.project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    let eight = ["d1", "d2", "d3", "d4", "d5", "d6", "d7", "d8"];
    let suite_path = test_dir.join("suite.v");
    std::fs::write(&suite_path, suite_with_tests(&eight)).unwrap();
    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session = compile_external_test_session(&workspace, &fixture.project_dir, "legion").expect("测试构建必须产出执行会话");
    let contracts = session.run_contracts.iter().map(|contract| contract.logical_entry.clone()).collect::<Vec<_>>();
    assert_eq!(discovered_test_names(&suite_path), contracts, "发现集合必须与执行合同一一对应");
}

#[test]
fn discovery_keeps_first_test_in_bom_prefixed_suite() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-discover-bom",
        r#"{
    name: "discover_bom_app",
    build: [ { target: "ovm" } ]
}
"#,
        PLAIN_MAIN,
    );
    let test_dir = fixture.project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    let eight = ["e1", "e2", "e3", "e4", "e5", "e6", "e7", "e8"];
    let suite_path = test_dir.join("suite.v");
    std::fs::write(&suite_path, format!("\u{feff}{}", suite_with_tests(&eight))).unwrap();
    assert_eq!(
        discovered_test_names(&suite_path),
        eight.to_vec(),
        "UTF-8 BOM 是文件头，不是 `[test]` 的分隔符，首个测试不得从运行集合里消失"
    );
}

#[test]
fn discovery_keeps_test_when_trivia_sits_between_attribute_and_declaration() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-discover-trivia",
        r#"{
    name: "discover_trivia_app",
    build: [ { target: "ovm" } ]
}
"#,
        PLAIN_MAIN,
    );
    let test_dir = fixture.project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    let suite_path = test_dir.join("suite.v");
    std::fs::write(
        &suite_path,
        "[test]\n# 首个测试的说明\n\nmicro first() -> i64 {\n    return 1\n}\n[test]\nmicro second() -> i64 {\n    return 2\n}\n",
    )
    .unwrap();
    assert_eq!(
        discovered_test_names(&suite_path),
        vec!["first".to_string(), "second".to_string()],
        "空白行/注释属于 trivia，不得切断 `[test]` 与其声明的配对"
    );
}
