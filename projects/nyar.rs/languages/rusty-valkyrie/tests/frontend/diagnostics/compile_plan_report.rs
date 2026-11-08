//! 块 5：端到端 —— `compile_plan` 失败时一次输出全部独立错误。
//!
//! 正例：同一编译单元三个未解析调用（含一个重复根因）全部出现在一次报告里，
//! 带 `file:line:col` 定位、源码片段、同根因归并备注与总数汇总；
//! 反例（旧行为锚点）：报告含全部独立错误而不是只第一条；失败路径暂存汇不残留。

use rusty_valkyrie::{
    compiler::diagnostics::pending,
    legion::{
        cmds::build::compile_plan,
        workspace::planner::{BuildRequest, WorkspaceResolver},
    },
};

fn create_multi_error_project(prefix: &str, source: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("create tempdir");
    let root = temp_dir.path();
    let project_dir = root.join("app");
    let source_dir = project_dir.join("source");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("legions.von"), "{\n    name: \"diag-smoke\",\n    members: [\n        \"app\"\n    ]\n}\n")
        .expect("write workspace manifest");
    std::fs::write(
        project_dir.join("legion.von"),
        "{\n    name: \"app\",\n    dependencies: {\n        \"std\": false,\n        \"core\": false\n    },\n    build: [\n        { target: \"ovm\" }\n    ]\n}\n",
    )
    .expect("write project manifest");
    // 该 fixture 关闭 std/core 依赖，primitive 声明只能由本包源码提供：
    // 与真实 std 包同形，使用测试侧唯一前导而不是逐文件抄写。
    std::fs::write(source_dir.join("primitives.v"), crate::primitives::PRIMITIVES).expect("write primitive declarations");
    std::fs::write(source_dir.join("main.v"), source).expect("write source");
    (temp_dir, project_dir)
}

#[test]
fn compile_plan_lists_every_independent_error_in_one_report() {
    pending::clear_pending_diagnostics();
    let (_temp, project_dir) = create_multi_error_project(
        "legion-multi-diag",
        "[main]\nmicro main() -> i64 {\n    alpha()\n    beta()\n    alpha()\n    return 0\n}\n",
    );
    let workspace = WorkspaceResolver::discover_for_project(&project_dir).expect("workspace discovery");
    let request = BuildRequest {
        project_dir: project_dir.clone(),
        target: "ovm".parse().expect("canonical target"),
        output_dir: None,
        ..Default::default()
    };
    let plan = workspace.build_plan(&request).expect("build plan");
    let message = compile_plan(&plan, false).expect_err("undeclared callees must fail").to_string();

    // 旧行为只报第一条；现在一次列出全部独立错误。
    assert!(message.contains("unresolved call `alpha`"), "{message}");
    assert!(message.contains("unresolved call `beta`"), "{message}");
    // 行列定位、代码片段、同根因归并备注与总数汇总齐备。
    assert!(message.contains("--> "), "expect file:line:col locations: {message}");
    assert!(message.contains('^'), "expect caret snippet: {message}");
    assert!(message.contains("more site with the same root cause"), "expect merge note: {message}");
    assert!(message.contains("error: aborting due to 3 previous errors"), "{message}");
    // 失败路径已取空暂存汇，不带残留。
    assert!(pending::pending_diagnostics_is_empty());
}
