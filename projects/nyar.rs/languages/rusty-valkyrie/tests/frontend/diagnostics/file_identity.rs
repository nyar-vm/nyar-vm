//! 块 6：源码身份的 per-file 合同。
//!
//! 每个物理文件拥有自己的诊断身份与独立 span 空间；解析恢复丢弃的项在批处理
//! 编译的闭包索引边界收口。正例：具名解析与组编译的诊断携带真实文件身份，跨三
//! 个文件的工程里单文件语法错误精确落到该文件的行列与片段；反例：匿名夹具保持
//! 匿名占位，干净工程不受收口影响照常编译。

use nyar_types::compiler_input::{CompilerSourceFile, CompilerSourceGroup};
use rusty_valkyrie::{
    compiler::{
        diagnostics::pending,
        hir::lowering::ValkyrieCompiler,
        syntax::oak::{parse_source, parse_source_with_identity},
    },
    legion::{
        cmds::build::compile_plan,
        workspace::planner::{BuildRequest, WorkspaceResolver},
    },
};

#[test]
fn named_parse_stamps_real_source_identity_on_recovery_diagnostics() {
    pending::clear_pending_diagnostics();
    parse_source_with_identity("micro broken() { + }\n", "source/probe.v").expect("recovery keeps the pipeline alive");
    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 1, "broken item records exactly one diagnostic: {collected:?}");
    assert_eq!(collected.diagnostics[0].row.source.id, "source/probe.v", "diagnostic must carry the real file identity");
    assert_eq!(collected.diagnostics[0].row.code, "valkyrie.frontend.parse.syntax");
}

#[test]
fn anonymous_parse_keeps_anonymous_placeholder() {
    pending::clear_pending_diagnostics();
    parse_source("micro broken() { + }\n").expect("recovery keeps the pipeline alive");
    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.diagnostics[0].row.source.id, "<frontend>", "in-memory fixture has no file, anonymous identity stays honest");
}

fn file(path: &str, source: &str) -> CompilerSourceFile {
    CompilerSourceFile { path: path.into(), source: source.into() }
}

fn entry_group(files: Vec<CompilerSourceFile>) -> CompilerSourceGroup {
    let mut group = CompilerSourceGroup::from_source_files("app", "app", files, Vec::new());
    group.is_entry = true;
    group
}

#[test]
fn group_compile_fails_closed_on_recovery_and_names_only_the_broken_file() {
    pending::clear_pending_diagnostics();
    let groups = vec![entry_group(vec![
        file("source/a.v", "[main]\nmicro main() -> i64 {\n    return 0\n}\n"),
        file("source/b.v", "micro broken() {\n    +\n}\n"),
        file("source/c.v", "micro helper() -> i64 {\n    return 1\n}\n"),
    ])];
    let error = ValkyrieCompiler::default().compile_source_groups_to_program(&groups).expect_err("恢复丢弃项必须在闭包索引边界收口");
    assert!(error.to_string().contains("recoverable error diagnostic"), "{error}");

    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 1, "only the broken file records a diagnostic: {collected:?}");
    let diagnostic = &collected.diagnostics[0];
    assert_eq!(diagnostic.row.source.id, "source/b.v", "diagnostic must name the broken file");
    assert_ne!(diagnostic.row.source.id, "source/a.v");
    assert_ne!(diagnostic.row.source.id, "source/c.v");
}

#[test]
fn clean_group_compile_is_not_affected_by_the_boundary() {
    pending::clear_pending_diagnostics();
    let groups = vec![entry_group(vec![
        file(
            "source/a.v",
            "namespace std.primitive;\n\n[primitive(\"std::primitive::i64\")] structure i64 { }\n\nnamespace app;\n\nusing std.primitive;\n\n[main]\nmicro main() -> i64 {\n    return 0\n}\n",
        ),
        file("source/c.v", "namespace app;\n\nusing std.primitive;\n\nmicro helper() -> i64 {\n    return 1\n}\n"),
    ])];
    ValkyrieCompiler::default().compile_source_groups_to_program(&groups).expect("干净工程必须照常编译");
    assert!(pending::pending_diagnostics_is_empty());
}

fn create_three_file_project(prefix: &str, broken: bool) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("create tempdir");
    let root = temp_dir.path();
    let project_dir = root.join("app");
    let source_dir = project_dir.join("source");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("legions.von"), "{\n    name: \"file-identity\",\n    members: [\n        \"app\"\n    ]\n}\n")
        .expect("write workspace manifest");
    std::fs::write(
        project_dir.join("legion.von"),
        "{\n    name: \"app\",\n    dependencies: {\n        \"std\": false,\n        \"core\": false\n    },\n    build: [\n        { target: \"ovm\" }\n    ]\n}\n",
    )
    .expect("write project manifest");
    std::fs::write(
        source_dir.join("a.v"),
        "namespace std.primitive;\n\n[primitive(\"std::primitive::i64\")] structure i64 { }\n\nnamespace app;\n\nusing std.primitive;\n\n[main]\nmicro main() -> i64 {\n    return 0\n}\n",
    )
    .expect("write a.v");
    std::fs::write(source_dir.join("c.v"), "namespace app;\n\nusing std.primitive;\n\nmicro helper() -> i64 {\n    return 1\n}\n")
        .expect("write c.v");
    let b_source = if broken {
        "micro broken() {\n    +\n}\n"
    }
    else {
        "using std.primitive;\n\nmicro fine() -> i64 {\n    return 2\n}\n"
    };
    std::fs::write(source_dir.join("b.v"), b_source).expect("write b.v");
    (temp_dir, project_dir)
}

fn build_message(project_dir: &std::path::Path) -> Result<String, String> {
    let workspace = WorkspaceResolver::discover_for_project(project_dir).expect("workspace discovery");
    let request = BuildRequest {
        project_dir: project_dir.to_path_buf(),
        target: "ovm".parse().expect("canonical target"),
        output_dir: None,
        ..Default::default()
    };
    let plan = workspace.build_plan(&request).expect("build plan");
    match compile_plan(&plan, false) {
        Ok(_) => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

#[test]
fn three_file_project_pinpoints_the_broken_file_with_line_column_and_snippet() {
    pending::clear_pending_diagnostics();
    let (_temp, project_dir) = create_three_file_project("three-file-broken", true);
    let message = build_message(&project_dir).expect_err("broken file must fail the build");

    // 报错必须精确指到 b.v 的行列与片段：`+` 在 b.v 第二行第五列。
    assert!(message.contains("valkyrie.frontend.parse.syntax"), "{message}");
    assert!(message.contains("--> source/b.v:2:5"), "{message}");
    assert!(message.contains("2 |     +"), "{message}");
    // 三个文件的偏移互不干扰：不得把 b.v 的错误归到 a.v / c.v。
    assert!(!message.contains("--> source/a.v"), "{message}");
    assert!(!message.contains("--> source/c.v"), "{message}");
    // 收口事实可见：最终失败行说明批处理编译因可恢复错误收口。
    assert!(message.contains("recoverable error diagnostic"), "{message}");
}

#[test]
fn three_file_project_builds_clean_when_no_file_is_broken() {
    pending::clear_pending_diagnostics();
    let (_temp, project_dir) = create_three_file_project("three-file-clean", false);
    build_message(&project_dir).expect("干净工程必须照常编译");
    assert!(pending::pending_diagnostics_is_empty());
}
