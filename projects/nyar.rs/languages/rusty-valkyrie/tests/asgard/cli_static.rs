//! CLI 路由测试：`asgard build` 的静态交付走静态站点编排，失败闭合且不回落到应用管线。

use std::{fs, path::PathBuf, process::ExitCode};

use rusty_valkyrie::asgard::cli::{AsgardBuildArgs, AsgardCommands, AsgardDelivery, run};
use tempfile::TempDir;

/// 带样式与初始状态绑定的根组件入口 fixture。
const FIXTURE: &str = r#"<widget index>
<div class="hero">
    <h1>{greeting}</h1>
</div>
</widget>
<script>
let greeting: utf8 = "你好，静态世界"
</script>
<style>.hero { color: #c67b34; }</style>
"#;

fn build_args(temp: &TempDir, entry: &str, title: Option<String>) -> AsgardBuildArgs {
    AsgardBuildArgs {
        project_dir: temp.path().to_path_buf(),
        output_dir: None,
        target: None,
        delivery: AsgardDelivery::Static { entry: PathBuf::from(entry), title },
    }
}

#[test]
fn build_routes_static_delivery_to_styled_page() {
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");

    let code = run(&AsgardCommands::Build(build_args(&temp, "source/index.w", Some("CLI 静态交付".into())))).expect("CLI 静态交付构建应成功");
    assert_eq!(code, ExitCode::SUCCESS);

    let html = fs::read_to_string(temp.path().join("dist").join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<!DOCTYPE html>"), "应为完整 HTML 页面");
    assert!(html.contains(".hero { color: #c67b34; }"), "应包含组件样式:\n{html}");
    assert!(html.contains("你好，静态世界"), "应包含初始状态插值:\n{html}");
    assert!(html.contains("<title>CLI 静态交付</title>"), "应使用显式标题:\n{html}");
}

#[test]
fn static_delivery_fails_closed_on_bad_entry() {
    // 非法入口后缀必须失败返回，而不是回落到应用构建管线。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.v"), FIXTURE).expect("写入入口文件失败");

    let error = run(&AsgardCommands::Build(build_args(&temp, "source/index.v", None))).expect_err("非法入口后缀应失败");
    assert!(error.to_string().contains(".w"), "诊断应说明合法后缀: {error}");
    assert!(!temp.path().join("dist").join("index.html").exists(), "失败路径不应产出页面");
}
