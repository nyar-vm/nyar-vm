use crate::support::{create_smoke_project, create_smoke_project_with_source};
use rusty_valkyrie::legion::cmds::{
    clean::{CleanArgs, run as run_clean},
    lint::{LintArgs, run as run_lint},
};
use std::{fs, process::ExitCode};

#[test]
fn clean_removes_dist_and_cache() {
    let fixture = create_smoke_project("legion-clean");
    let dist = fixture.project_dir.join("dist");
    let cache = fixture.project_dir.join(".cache");
    fs::create_dir_all(&dist).unwrap();
    fs::create_dir_all(&cache).unwrap();
    fs::write(dist.join("artifact.txt"), "x").unwrap();

    assert_eq!(run_clean(&CleanArgs { project_dir: fixture.project_dir.clone(), workspace: false }).unwrap(), ExitCode::SUCCESS);
    assert!(!dist.exists());
    assert!(!cache.exists());
}

#[test]
fn lint_accepts_valid_source() {
    let fixture = create_smoke_project_with_source(
        "legion-lint",
        &crate::primitives::with_primitives(
            r#"micro main() -> i64 {
    return 0;
}
"#,
        ),
    );

    assert_eq!(
        run_lint(&LintArgs { project_dir: fixture.project_dir.clone(), workspace: false, format: None, fix: false }).unwrap(),
        ExitCode::SUCCESS
    );
}

#[test]
fn lint_awsl_recognizes_w_extension() {
    let fixture = create_smoke_project("legion-lint-awsl-w");
    let source_dir = fixture.project_dir.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("index.w"), "<template>\n<div>{title}</div>\n</template>\n").unwrap();

    assert_eq!(
        run_lint(&LintArgs { project_dir: fixture.project_dir.clone(), workspace: false, format: Some("awsl".into()), fix: false }).unwrap(),
        ExitCode::SUCCESS,
        "干净的 `.w` 应通过 AWSL lint"
    );

    // `.w` 必须真正进入 AWSL lint 而非被静默跳过：script 内冗余括号应报样式问题。
    fs::write(source_dir.join("bad.w"), "<script>\nif (x) {\n  y()\n}\n</script>\n").unwrap();
    let result = run_lint(&LintArgs { project_dir: fixture.project_dir.clone(), workspace: false, format: Some("awsl".into()), fix: false });
    match result {
        Err(error) => assert!(error.to_string().contains("awsl style issues"), "`.w` 样式问题应被捕获: {error}"),
        Ok(_) => panic!("`.w` 内的样式问题不应被静默跳过"),
    }
}
