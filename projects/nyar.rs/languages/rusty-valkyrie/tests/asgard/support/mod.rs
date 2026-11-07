//! 静态构建测试的共享工程 fixture：含 `asgard.render` 合同族的 Legion 工作区。
//!
//! SSG 渲染经 OVM 执行（`asgard.render` 合同族 + `asgard.render.host.ovm`
//! provider），测试工程必须声明 `ovm` 目标与合同族依赖；
//! 浏览器交付链同时需要 `wasm32-unknown-browser-wasm` 目标。

use std::path::PathBuf;

/// `valkyrie.v` 语料根：优先 `VALKYRIE_V_ROOT`，否则取本仓兄弟检出。
///
/// 与 `tests/legion` 的语料定位同一纪律：CI 在仓库根旁检出固定 rev 的语料，
/// 本地开发者可直接指向任意干净快照。
pub fn valkyrie_v_root() -> PathBuf {
    std::env::var("VALKYRIE_V_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..").join("valkyrie.v"))
}

/// `valkyrie.v` 兄弟检出中的包绝对路径（正斜杠形态，供清单/工作区成员引用）。
pub fn valkyrie_v_package(parts: &[&str]) -> String {
    let path = parts.iter().fold(valkyrie_v_root(), |path, part| path.join(part));
    path.to_string_lossy().replace('\\', "/")
}

/// 项目清单：声明合同族依赖（workspace 成员）与浏览器 + OVM 双目标。
pub fn legion_manifest(name: &str) -> String {
    format!(
        r#"{{
    name: "{name}",
    version: "0.1.0",
    dependencies: {{
        "asgard.render": {{ version: "workspace" }},
        "asgard.render.host.ovm": {{ version: "workspace" }}
    }},
    build: [
        {{ target: "wasm32-unknown-browser-wasm" }},
        {{ target: "ovm" }}
    ]
}}
"#
    )
}

/// 工作区清单：把 `asgard.render` 合同族两个包注册为 workspace 成员。
pub fn workspace_manifest(name: &str) -> String {
    format!(
        r#"{{
    name: "{name}-ws",
    members: [
        "{}",
        "{}"
    ]
}}
"#,
        valkyrie_v_package(&["projects", "asgard._", "projects", "asgard.render"]),
        valkyrie_v_package(&["projects", "asgard._", "projects", "asgard.render.host.ovm"])
    )
}

/// 写入工作区清单与项目清单（不含 `asgard.config.v`，各测试自行决定）。
pub fn write_legion_project(project: &std::path::Path, name: &str) {
    std::fs::write(project.join("legions.von"), workspace_manifest(name)).expect("写入 legions.von 失败");
    std::fs::write(project.join("legion.von"), legion_manifest(name)).expect("写入 legion.von 失败");
}
