//! `manifest.json` 生成。

use std::collections::BTreeMap;

use serde::Serialize;

use crate::asgard::{
    awsl::LoweredComponent,
    codegen::{JsGlueOutput, encode_mobile_ui_package},
};

/// manifest 输出。
#[derive(Debug, Clone)]
pub struct ManifestOutput {
    /// JSON 文本。
    pub content: String,
    /// 相对路径。
    pub relative_path: String,
}

/// 单个产物文件的内容摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDigest {
    /// manifest 相对产物路径（如 `c/index.js`、`app.wasm`）。
    pub path: String,
    /// 小写十六进制 SHA-256；由真实写出产物按字节计算。
    pub sha256: String,
}

/// Asset URL style in manifest entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestUrlMode {
    /// Root-absolute paths (`/c/foo.js`) for served apps.
    Absolute,
    /// Relative paths (`c/foo.js`) for `file://` report previews.
    Relative,
}

/// 生成 manifest.json，可切换 relative / absolute asset URLs。
///
/// `css_files` 是打包阶段真实落盘的 css chunk 文件名（顺序即产物顺序）；
/// `per_component_css` 为真时每个组件条目额外登记自己的 css chunk URL。
/// manifest 不再按模块名推算 css 文件名，避免与实际产物漂移。
pub fn generate_manifest_with_urls(
    module_stem: &str,
    mode: &str,
    components: &[LoweredComponent],
    js_outputs: &[JsGlueOutput],
    wasm_name: &str,
    wasm_built: bool,
    url_mode: ManifestUrlMode,
    wasm_content_hash: Option<&str>,
    digests: &[ManifestDigest],
    css_files: &[String],
    per_component_css: bool,
    porting_cost: &[String],
) -> ManifestOutput {
    let module = module_stem.to_string();
    let css = css_files.iter().map(|file| url_path(&file.replace('\\', "/"), url_mode)).collect();

    let wasm = if wasm_built {
        vec![WasmEntry {
            name: module.clone(),
            url: url_path(&format!("{wasm_name}.wasm"), url_mode),
            glue: url_path(&format!("{wasm_name}.mjs"), url_mode),
            hash: wasm_content_hash.map(str::to_string),
        }]
    }
    else {
        vec![]
    };

    // 摘要按路径字典序序列化：同一产物树在任何构建批次都得到同一 manifest。
    let integrity = digests.iter().map(|digest| (digest.path.clone(), digest.sha256.clone())).collect();

    let manifest = VoaManifest {
        module: module.clone(),
        mode: mode.to_string(),
        css,
        wasm,
        integrity,
        porting_cost: porting_cost.to_vec(),
        render_ir: Some(RenderIrManifest { encoding: "asgard-ui-v1".into(), size: encode_mobile_ui_package(components).len() }),
        components: components
            .iter()
            .zip(js_outputs.iter())
            .map(|(component, js)| {
                let component_css = if per_component_css {
                    let filename = component_css_filename(&component.name);
                    css_files.contains(&filename).then(|| url_path(&filename, url_mode))
                }
                else {
                    None
                };
                ManifestComponent {
                    name: js_relative_name(&js.relative_path),
                    js: url_path(&js.relative_path.replace('\\', "/"), url_mode),
                    css: component_css,
                    strategy: component.hydrate_strategy.clone(),
                    island: component.island_type.clone(),
                }
            })
            .collect(),
    };

    let content = serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".into());
    ManifestOutput { content, relative_path: "manifest.json".into() }
}

/// 组件级 css chunk 文件名：与 `extract_and_merge_styles` 的 per-component 命名一致。
pub fn component_css_filename(component_name: &str) -> String {
    format!("{}.css", component_name.to_ascii_lowercase())
}

fn url_path(path: &str, mode: ManifestUrlMode) -> String {
    let trimmed = path.trim_start_matches('/');
    match mode {
        ManifestUrlMode::Absolute => format!("/{trimmed}"),
        ManifestUrlMode::Relative => trimmed.to_string(),
    }
}

fn js_relative_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).trim_end_matches(".js").to_string()
}

#[derive(Serialize)]
struct VoaManifest {
    module: String,
    mode: String,
    css: Vec<String>,
    wasm: Vec<WasmEntry>,
    /// 内容摘要表：manifest 相对路径 → 小写十六进制 SHA-256。
    integrity: BTreeMap<String, String>,
    /// 移植成本条目：`web_only` 原始 CSS 块登记；非 dom 后端构建据此 fail-closed。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    porting_cost: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render_ir: Option<RenderIrManifest>,
    components: Vec<ManifestComponent>,
}

#[derive(Serialize)]
struct RenderIrManifest {
    encoding: String,
    size: usize,
}

#[derive(Serialize)]
struct WasmEntry {
    name: String,
    url: String,
    glue: String,
    /// 产物字节摘要（`sha256:<hex>`）；仅当调用方持有已落盘 chunk 时写入。
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

#[derive(Serialize)]
struct ManifestComponent {
    name: String,
    js: String,
    /// 组件级 css chunk URL；仅按组件切分样式时登记。
    #[serde(skip_serializing_if = "Option::is_none")]
    css: Option<String>,
    strategy: String,
    island: String,
}
