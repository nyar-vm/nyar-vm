//! `.w` / `.awsl` 源码发现与身份碰撞测试。

use rusty_valkyrie::asgard::pipeline::discover_sources;
use std::fs;
use tempfile::TempDir;

/// 在临时目录中构造一个含 `source/` 的项目骨架，返回 TempDir（drop 时自动清理）。
fn make_project(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("创建临时目录失败");
    let source = dir.path().join("source");
    fs::create_dir_all(&source).expect("创建 source/ 目录失败");
    for (relative, content) in files {
        let path = source.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写入测试文件失败");
    }
    dir
}

#[test]
fn discovers_awsl_and_w_with_correct_component_names() {
    // `.awsl` 与 `.w` 同语义，都应进入发现结果。
    let project = make_project(&[("home.awsl", "<widget><div>home</div></widget>"), ("about.w", "<widget><div>about</div></widget>")]);
    let found = discover_sources(project.path()).expect("发现源码失败");
    assert_eq!(found.awsl_files.len(), 2, "两种后缀都应被发现");

    let home = found.awsl_files.iter().find(|f| f.component_name == "home").expect("缺少 home 组件");
    assert_eq!(home.relative_path, "home.awsl");
    let about = found.awsl_files.iter().find(|f| f.component_name == "about").expect("缺少 about 组件");
    assert_eq!(about.relative_path, "about.w");
}

#[test]
fn same_stem_different_ext_in_one_dir_is_identity_collision() {
    // 同一父目录下 foo.w 与 foo.awsl：不建立覆盖关系，必须诊断报错。
    let project = make_project(&[("foo.awsl", "<widget><div>a</div></widget>"), ("foo.w", "<widget><div>b</div></widget>")]);
    let error = discover_sources(project.path()).expect_err("同目录同名不同后缀应报身份碰撞");
    let message = error.to_string();
    assert!(message.contains("foo.awsl"), "错误信息应包含相对路径 foo.awsl: {message}");
    assert!(message.contains("foo.w"), "错误信息应包含相对路径 foo.w: {message}");
}

#[test]
fn same_stem_in_different_dirs_is_ok() {
    // 不同目录下的同名文件不构成身份碰撞。
    let project = make_project(&[("pages/a/foo.w", "<widget><div>a</div></widget>"), ("pages/b/foo.awsl", "<widget><div>b</div></widget>")]);
    let found = discover_sources(project.path()).expect("不同目录同名不应碰撞");
    assert_eq!(found.awsl_files.len(), 2);
    assert!(found.awsl_files.iter().all(|f| f.component_name == "foo"));
}
