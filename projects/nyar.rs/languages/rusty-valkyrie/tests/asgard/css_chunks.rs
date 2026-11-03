//! CSS chunk 切分合同：样式按组件切分为独立 chunk，manifest 逐条登记并关联组件，
//! 摘要覆盖每个 chunk 的真实字节；同一组件集在 `merge` 模式下仍是单一全局文件。
//!
//! 前后对比由两个测试并排固化：`merge_mode_emits_single_global_stylesheet`
//! （切分前：1 个全局 css）与 `per_component_mode_emits_one_chunk_per_component`
//! （切分后：每组件 1 个 chunk + manifest 条目/关联/摘要）。

use std::{fs, path::Path};

use rusty_valkyrie::asgard::{
    awsl::{LoweredComponent, LoweringOptions, compile_awsl_source},
    codegen::{IslandPackageOptions, package_browser_islands},
};
use tempfile::TempDir;

/// 两个带各自样式的组件：切分后应各得一个 css chunk。
fn fixture_components() -> Vec<LoweredComponent> {
    let index = compile_awsl_source(
        "<widget index>\n<Text>index</Text>\n</widget>\n<style>\nroot { background: @surface }\n</style>\n",
        "index",
        "index.w",
        &LoweringOptions::default(),
    )
    .expect("index 组件降级应成功");
    let panel = compile_awsl_source(
        "<widget panel>\n<Text>panel</Text>\n</widget>\n<style>\nroot { background: @panel }\n</style>\n",
        "panel",
        "panel.w",
        &LoweringOptions::default(),
    )
    .expect("panel 组件降级应成功");
    vec![index, panel]
}

fn package(components: &[LoweredComponent], output: &Path, css_mode: &str) {
    let options = IslandPackageOptions {
        module_stem: "css-chunks".into(),
        wasm_stem: "css-chunks".into(),
        wasm_built: false,
        relative_urls: true,
        css_mode: css_mode.into(),
        mode: "static".into(),
        tailwind_css: None,
        porting_cost: Vec::new(),
    };
    package_browser_islands(components, output, &options).expect("岛打包应成功");
}

fn read_manifest(output: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(output.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON")
}

#[test]
fn merge_mode_emits_single_global_stylesheet() {
    // 切分前的事实：全部组件样式合并进单一全局 css 文件。
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("out");
    package(&fixture_components(), &output, "merge");
    let merged = fs::read_to_string(output.join("css-chunks.css")).expect("merge 模式应输出全局 css");
    assert!(merged.contains(".sc-root { background: #0f172a; }"), "全局 css 应含第一个组件的合同降级:\n{merged}");
    assert!(merged.contains(".sc-root { background: #1e293b; }"), "全局 css 应含第二个组件的合同降级:\n{merged}");
    assert!(!output.join("index.css").exists(), "merge 模式不得输出组件级 chunk");
    let manifest = read_manifest(&output);
    let css = manifest.get("css").and_then(serde_json::Value::as_array).expect("manifest 应声明 css");
    assert_eq!(css.len(), 1, "merge 模式 manifest 应恰有 1 条 css");
}

#[test]
fn per_component_mode_emits_one_chunk_per_component() {
    // 切分后：每组件一个 css chunk，字节各自独立，manifest 逐条登记并关联组件。
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("out");
    let components = fixture_components();
    package(&components, &output, "per-component");

    let index_css = fs::read_to_string(output.join("index.css")).expect("index 组件应有独立 css chunk");
    let panel_css = fs::read_to_string(output.join("panel.css")).expect("panel 组件应有独立 css chunk");
    assert!(index_css.contains("background: #0f172a"), "index chunk 只含自己的合同降级:\n{index_css}");
    assert!(panel_css.contains("background: #1e293b"), "panel chunk 只含自己的合同降级:\n{panel_css}");
    assert!(!index_css.contains("#1e293b"), "chunk 之间不得串样式:\n{index_css}");
    assert!(!output.join("css-chunks.css").exists(), "per-component 模式不得再输出全局合并文件");
    let index_bytes = fs::metadata(output.join("index.css")).expect("index.css 元数据").len();
    let panel_bytes = fs::metadata(output.join("panel.css")).expect("panel.css 元数据").len();
    assert!(index_bytes > 0 && panel_bytes > 0, "每个 chunk 必须有真实字节");

    let manifest = read_manifest(&output);
    let css = manifest.get("css").and_then(serde_json::Value::as_array).expect("manifest 应声明 css chunk");
    let css_names: Vec<&str> = css.iter().map(|value| value.as_str().expect("css 条目应为字符串")).collect();
    assert_eq!(css_names, vec!["index.css", "panel.css"], "manifest 必须按产物顺序登记每个 css chunk");

    let entries = manifest.get("components").and_then(serde_json::Value::as_array).expect("manifest 应声明组件");
    assert_eq!(entries.len(), 2, "应登记两个组件");
    assert_eq!(entries[0].get("css").and_then(serde_json::Value::as_str), Some("index.css"), "index 组件必须关联自己的 css chunk");
    assert_eq!(entries[1].get("css").and_then(serde_json::Value::as_str), Some("panel.css"), "panel 组件必须关联自己的 css chunk");

    let integrity = manifest.get("integrity").and_then(serde_json::Value::as_object).expect("manifest 必须声明摘要表");
    for (path, bytes) in [("index.css", index_bytes), ("panel.css", panel_bytes)] {
        let declared = integrity.get(path).and_then(serde_json::Value::as_str).unwrap_or_else(|| panic!("摘要必须覆盖 {path}"));
        let actual = fs::read(output.join(path)).expect("读取 chunk 失败");
        let expected: String = sha2(&actual);
        assert_eq!(declared, &expected, "{path}（{bytes} 字节）的摘要必须等于真实字节的 SHA-256");
    }
}

fn sha2(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}
