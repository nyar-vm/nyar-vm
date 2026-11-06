//! 交互产物合同：静态交付三件套必须携带真实事件/状态接线，而不只是文件存在。
//!
//! 夹具含 `@click` 事件、`let mut` 响应式状态与状态改写 handler；产物侧的
//! 可验证事实：wasm chunk 导出 handler 调用符号（`awsl_call_<handler>`）与
//! 响应式信号读取符号（`awsl_sig_<route>_<binding>`），页面以初始状态渲染
//! 并声明岛挂载点，boot 运行时经 `callExport` 消费这些导出。浏览器内的真实
//! 点击/状态往返由产物之上的端到端验证覆盖，本测试锁定产物内的接线事实。

use std::{fs, path::PathBuf};

use rusty_valkyrie::asgard::static_site::{StaticSiteOptions, build_static_site};
use tempfile::TempDir;

use crate::support::write_legion_project;

/// 带事件与响应式状态的根组件入口夹具。
const FIXTURE: &str = r#"<widget index>
<Column class="hero">
    <Text class="hero-title">{greeting}</Text>
    <Text class="state">{mode}</Text>
    <Text class="bump" @click="toggle('on')">toggle</Text>
</Column>
</widget>
<script>
let greeting: utf8 = "交互产物"
let mut mode: utf8 = "off"
micro toggle(key: utf8) {
    mode = "on"
}
</script>
<style>
root { color: @brand }
</style>
"#;

/// 构建交互夹具并返回输出目录。
fn build_interactive(project: &std::path::Path, output: &std::path::Path) {
    fs::create_dir_all(project.join("source")).expect("创建 source/ 目录失败");
    write_legion_project(project, "interactive-artifact");
    fs::write(project.join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    build_static_site(&StaticSiteOptions {
        project_dir: project.to_path_buf(),
        entry: PathBuf::from("source/index.w"),
        output_dir: Some(output.to_path_buf()),
        title: Some("交互产物".into()),
        base: "/".to_string(),
        assets_dir: None,
    })
    .expect("交互夹具的静态站点构建应成功");
}

#[test]
fn interactive_build_emits_paired_browser_artifacts() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("out");
    build_interactive(temp.path(), &output);
    let expected = [
        "boot.js",
        "c/index.js",
        "index.css",
        "index.html",
        "interactive-artifact.mjs",
        "interactive-artifact.wasm",
        "interactive-artifact.wat",
        "manifest.json",
    ];
    let mut files: Vec<String> = Vec::new();
    collect_files(&output, &output, &mut files);
    files.sort();
    assert_eq!(files, expected, "交互产物树应为页面 + 成对浏览器产物，实际：{files:?}");
}

fn collect_files(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("读取输出目录失败") {
        let path = entry.expect("目录项").path();
        if path.is_dir() {
            collect_files(root, &path, out);
        }
        else {
            out.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn wasm_chunk_exports_event_handler_and_signal_symbols() {
    // 事件 handler 与响应式状态必须落成 wasm 导出符号（命名合同见 `ui_host_abi`）：
    // 这是「点击 → handler → 状态」链路在产物内的可验证接线，不是名字表放行。
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("out");
    build_interactive(temp.path(), &output);
    let wat = fs::read_to_string(output.join("interactive-artifact.wat")).expect("读取 wat 失败");
    assert!(wat.contains("awsl_call_toggle"), "wasm 必须导出事件 handler 调用符号:\n 期望含 `awsl_call_toggle`");
    assert!(wat.contains("awsl_sig_index_mode"), "wasm 必须导出响应式状态信号符号:\n 期望含 `awsl_sig_index_mode`");
}

#[test]
fn page_renders_initial_state_and_island_mount() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("out");
    build_interactive(temp.path(), &output);
    let html = fs::read_to_string(output.join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("交互产物"), "初始状态文本应渲染进页面:\n{html}");
    assert!(html.contains(">off<"), "响应式状态初始值应渲染进页面:\n{html}");
    assert!(html.contains("data-component=\"index\""), "页面应声明岛挂载点供 hydration:\n{html}");
    assert!(html.contains("<script src=\"boot.js\"></script>"), "页面应引用 boot 运行时:\n{html}");
}

#[test]
fn boot_runtime_consumes_call_export_contract() {
    // boot 运行时是产物内唯一经 `callExport` 消费 wasm 导出的脚本；
    // 组件胶水经它注册，事件/状态接线才可能在浏览器内闭合。
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("out");
    build_interactive(temp.path(), &output);
    let boot = fs::read_to_string(output.join("boot.js")).expect("读取 boot.js 失败");
    assert!(boot.contains("callExport"), "boot 运行时必须携带 callExport 合同");
    let component_glue = fs::read_to_string(output.join("c/index.js")).expect("读取组件胶水失败");
    assert!(component_glue.contains("index"), "组件胶水应注册入口组件:\n{component_glue}");
}
