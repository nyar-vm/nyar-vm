//! 静态站点构建编排。
//!
//! 职责：把既有所有权层串成一次可被 CLI / 测试调用的静态站点构建——
//! 显式入口校验（后缀集合取自单一事实源 `nyar_format::formatter::AWSL_EXTENSIONS`：
//! `.awsl` 规范拼写、`.w` 同义缩写，两种后缀同格式同语义，不做后缀猜测，
//! 入口必须显式给出且位于项目根内）、AWSL 解析与降级、DOM 静态渲染
//! （以组件初始状态渲染，外部数据为 `Null`）、页面组装、WASM chunk 与岛包产物装配。
//! 本模块不做表达式求值，也不复制解析、降级或渲染逻辑；
//! 各环节的唯一事实所有者分别是 `awsl::compile`、`codegen::html_render`、`ssg`、
//! `compile::compile_wasm_bundle` 与 `codegen::package_browser_islands`。
//!
//! 产物树（`dist/`）：`index.html`（含 `data-component` 岛挂载点与 `boot.js` 引用）、
//! `<module>.wasm`、`<module>.mjs`（浏览器 WASM glue）、`c/<route>.js`（组件胶水）、
//! `manifest.json`、`boot.js`、`<module>.css`。页面用相对 URL，子目录部署由
//! `<base href>`（非根基址）解析。

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::{
    asgard::{
        awsl::{LoweringOptions, compile_awsl_source},
        codegen::{
            IslandPackageOptions, StaticRenderResult, asgard_boot_script_tag, asgard_boot_stylesheet_tag, build_awsl_wasm_source,
            package_browser_islands, render_static_ir,
        },
        compile::{GeneratedEntrySurface, GeneratedSource, resolve_project_source_groups},
        error::{AsgardError, AsgardResult},
        ssg::generate_static_page_full,
        wasm::{compile_wasm_bundle, copy_wasm_artifacts_to_dist},
    },
    legion::CanonicalTarget,
};

/// 静态站点构建选项。
#[derive(Debug, Clone)]
pub struct StaticSiteOptions {
    /// 项目根目录。
    pub project_dir: PathBuf,
    /// 入口根组件文件路径（相对 `project_dir`，如 `source/index.w`）。
    /// `.awsl` 为规范拼写，`.w` 为同义缩写，两者同格式同语义；须显式给出，不做后缀猜测。
    pub entry: PathBuf,
    /// 输出目录；缺省为 `project_dir/dist`。
    pub output_dir: Option<PathBuf>,
    /// 页面 `<title>` 文本；`None` 时使用根组件名（入口文件 stem）。
    pub title: Option<String>,
    /// 产物基址（目录 URL，由配置所有者规范化）；缺省 `/` 表示根部署。
    ///
    /// 非根基址写入页面 `<base href>`，支持子目录部署；本层只消费已解析基址，
    /// 不重新解释 `artifact.base` 配置字段。
    pub base: String,
    /// 显式资源目录（相对 `project_dir`）；`None` 表示不拷贝资源。
    ///
    /// 资源递归复制到产物根，与页面 / chunk / 胶水 / manifest 同名时失败闭合，
    /// 不静默覆盖产物。
    pub assets_dir: Option<PathBuf>,
}

/// 静态站点构建成功报告。
#[derive(Debug, Clone)]
pub struct StaticSiteReport {
    /// 实际写出产物的目录。
    pub output_dir: PathBuf,
    /// 产物 `index.html` 的完整路径。
    pub page_path: PathBuf,
    /// 根组件名（来自入口文件 stem，widget 名与路由名的唯一来源）。
    pub component_name: String,
}

/// 构建静态站点：入口校验 → AWSL 降级 → DOM 静态渲染 → 页面组装 → 写出 `index.html`。
pub fn build_static_site(options: &StaticSiteOptions) -> AsgardResult<StaticSiteReport> {
    let extension = options.entry.extension().and_then(|ext| ext.to_str()).unwrap_or_default();
    if !nyar_format::formatter::is_awsl_extension(extension) {
        return Err(AsgardError::msg(report!(
            "静态站点入口必须是 `.awsl`（规范拼写）或同义缩写 `.w` 文件（两种后缀同格式同语义，不做后缀猜测）: {}",
            options.entry.display()
        )));
    }

    // 入口合同正式前置校验：项目根包含性以文件系统规范化（`canonicalize` 解析 `..` 段与
    // 符号链接）判定，先于读取入口内容失败闭合。词法 `starts_with` 在 Unix 上不折叠 `..`，
    // 不能充当包含性证据。
    let project_root = std::path::absolute(&options.project_dir)
        .map_err(|error| AsgardError::msg(report!("规范化项目根失败 {}: {error}", options.project_dir.display())))?;
    let entry_path = std::path::absolute(project_root.join(&options.entry))
        .map_err(|error| AsgardError::msg(report!("规范化入口路径失败 {}: {error}", options.entry.display())))?;
    let root_canonical =
        std::fs::canonicalize(&project_root).map_err(|error| AsgardError::msg(report!("项目根不可用 {}: {error}", project_root.display())))?;
    // 入口可能尚不存在（缺失文件应报读取失败并带路径证据）：以父目录规范化结果重建
    // 判定基准；父目录也不存在时无从判定包含性，同样由后续读取失败闭合。
    let entry_canonical = std::fs::canonicalize(&entry_path).ok().or_else(|| {
        let parent = entry_path.parent()?;
        let name = entry_path.file_name()?;
        Some(std::fs::canonicalize(parent).ok()?.join(name))
    });
    if let Some(resolved) = &entry_canonical {
        if !resolved.starts_with(&root_canonical) {
            return Err(AsgardError::msg(report!("入口越出项目根: {} 不在 {} 内", resolved.display(), project_root.display())));
        }
    }

    let file_stem = entry_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| AsgardError::msg(report!("入口文件名无法提取组件名: {}", entry_path.display())))?;
    let relative_path = entry_path.strip_prefix(&project_root).unwrap_or(&entry_path).to_string_lossy().replace('\\', "/");

    let source = std::fs::read_to_string(&entry_path)
        .map_err(|error| AsgardError::msg(report!("读取入口文件失败 {}: {error}", entry_path.display())))?;
    let component = compile_awsl_source(&source, file_stem, &relative_path, &LoweringOptions::default())
        .map_err(|error| AsgardError::msg(report!("{}:{}: {}", entry_path.display(), error.span.start, error.message)))?;

    // 静态页面以组件初始状态渲染，作为 hydration 前的占位内容；外部数据为 Null。
    let rendered = render_static_ir(&component, &serde_json::Value::Null);
    let title = options.title.as_deref().unwrap_or(file_stem);
    let base = page_base(&options.base)?;

    let output_dir = options.output_dir.clone().unwrap_or_else(|| project_root.join("dist"));
    std::fs::create_dir_all(&output_dir).map_err(|error| AsgardError::msg(report!("创建输出目录失败 {}: {error}", output_dir.display())))?;

    // 浏览器交付产物：WASM chunk、浏览器 glue、组件胶水、boot 与 manifest。
    // 命名、URL 形态与初始化顺序都由产物所有者决定，本层只编排调用次序。
    let target = CanonicalTarget::browser();
    let generated_source = build_awsl_wasm_source(std::slice::from_ref(&component));
    let resolved = resolve_project_source_groups(&project_root, &target)
        .map_err(|error| AsgardError::msg(report!("解析项目源码组失败（静态交付要求可解析的 Legion 工程）: {error}")))?;
    let module_stem = resolved.project_name.replace('.', "-");

    let wasm_report = compile_wasm_bundle(
        &resolved.groups,
        GeneratedSource { producer: "asgard.awsl.static-site", source: &generated_source },
        &output_dir,
        &module_stem,
        &target,
        GeneratedEntrySurface::GeneratedOnly,
    )
    .map_err(|error| AsgardError::msg(report!("WASM 编译失败: {error}")))?;
    copy_wasm_artifacts_to_dist(&output_dir, &wasm_report)?;

    let wasm_stem = wasm_report.wasm_filename.trim_end_matches(".wasm").to_string();
    let package_options = IslandPackageOptions {
        module_stem: module_stem.clone(),
        wasm_stem,
        wasm_built: true,
        // 页面与产物同处部署目录：相对 URL 让子目录部署由 `<base href>` 解析。
        relative_urls: true,
        css_mode: "merge".into(),
        mode: "static".into(),
        tailwind_css: None,
    };
    let package_report = package_browser_islands(std::slice::from_ref(&component), &output_dir, &package_options)?;
    let stylesheet_href = package_report.css_filenames.first().cloned().unwrap_or_default();

    // 页面只声明岛挂载点与 boot 脚本：组件 DOM 由 WASM 侧的 `awsl_hydrate_*` 建出，
    // 预渲染内容留在挂载点内作为无 JS 时的占位，hydration 时由 boot 清空后替换。
    let island = format!(
        "<div data-island=\"hydrated\" data-component=\"{route}\" data-module=\"{module}\">{html}</div>",
        route = component.route_name,
        module = module_stem,
        html = rendered.html,
    );
    let head_tags = if stylesheet_href.is_empty() { String::new() } else { format!("  {}\n", asgard_boot_stylesheet_tag(&stylesheet_href)) };
    let results = [StaticRenderResult { html: island, css: rendered.css, scope: rendered.scope, diagnostics: rendered.diagnostics }];
    let page = generate_static_page_full(title, &results, "", &base, &head_tags, asgard_boot_script_tag(true));

    let page_path = output_dir.join("index.html");
    std::fs::write(&page_path, page).map_err(|error| AsgardError::msg(report!("写入页面失败 {}: {error}", page_path.display())))?;

    // 资源目录最后复制：前面的产物文件构成保留名集合，重名即失败闭合。
    if let Some(assets_dir) = &options.assets_dir {
        let assets_absolute = std::path::absolute(project_root.join(assets_dir))
            .map_err(|error| AsgardError::msg(report!("规范化资源目录失败 {}: {error}", assets_dir.display())))?;
        // 包含性以文件系统规范化为准（与入口同一判据），词法比较在 Windows 上会误判。
        let assets_root = std::fs::canonicalize(&assets_absolute)
            .map_err(|error| AsgardError::msg(report!("资源目录不可用 {}: {error}", assets_absolute.display())))?;
        if !assets_root.starts_with(&root_canonical) {
            return Err(AsgardError::msg(report!("资源目录越出项目根: {} 不在 {} 内", assets_root.display(), project_root.display())));
        }
        let mut reserved = BTreeSet::new();
        reserved.insert("index.html".to_string());
        reserved.insert("boot.js".to_string());
        reserved.insert("manifest.json".to_string());
        reserved.insert(wasm_report.wasm_filename.clone());
        reserved.insert(wasm_report.glue_filename.clone());
        for css_filename in &package_report.css_filenames {
            reserved.insert(css_filename.clone());
        }
        reserved.insert(format!("c/{}.js", component.route_name));
        copy_assets_into_output(&assets_root, &output_dir, &reserved)?;
    }

    Ok(StaticSiteReport { output_dir, page_path, component_name: file_stem.to_string() })
}

/// 递归复制资源目录到产物根；与已生成产物同名的条目失败闭合，不静默覆盖。
fn copy_assets_into_output(assets_root: &Path, output_dir: &Path, reserved: &BTreeSet<String>) -> AsgardResult<()> {
    let mut pending = vec![assets_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries =
            std::fs::read_dir(&directory).map_err(|error| AsgardError::msg(report!("读取资源目录失败 {}: {error}", directory.display())))?;
        for entry in entries {
            let entry = entry.map_err(|error| AsgardError::msg(report!("读取资源目录项失败: {error}")))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(assets_root)
                .map_err(|error| AsgardError::msg(report!("资源路径不可相对化 {}: {error}", path.display())))?
                .to_string_lossy()
                .replace('\\', "/");
            if reserved.contains(&relative) {
                return Err(AsgardError::msg(report!("资源 {} 与已生成产物同名，拒绝覆盖", relative)));
            }
            let destination = output_dir.join(&relative);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|error| AsgardError::msg(report!("创建资源目录失败 {}: {error}", parent.display())))?;
            }
            std::fs::copy(&path, &destination)
                .map_err(|error| AsgardError::msg(report!("复制资源失败 {} → {}: {error}", path.display(), destination.display())))?;
        }
    }
    Ok(())
}

/// 校验并规范化为可写入 `<base href>` 的目录 URL。
///
/// 只检查消费侧合同（非空、以 `/` 开头、不含 `..` 段），末尾补 `/`；根 `/` 保持不变。
/// `artifact.base` 的默认值与用户拼写规范化由配置所有者负责，本层不重复实现。
fn page_base(base: &str) -> AsgardResult<String> {
    let trimmed = base.trim();
    if trimmed.is_empty() {
        return Err(AsgardError::msg(report!("静态站点基址不能为空；缺省根部署请使用 `/`")));
    }
    if !trimmed.starts_with('/') {
        return Err(AsgardError::msg(report!("静态站点基址必须是目录 URL（以 `/` 开头）: {trimmed}")));
    }
    if trimmed.split('/').any(|segment| segment == "..") {
        return Err(AsgardError::msg(report!("静态站点基址不得包含 `..` 段: {trimmed}")));
    }
    if trimmed == "/" {
        return Ok("/".to_string());
    }
    if trimmed.ends_with('/') {
        return Ok(trimmed.to_string());
    }
    Ok(format!("{trimmed}/"))
}
