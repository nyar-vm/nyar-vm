//! 报告混合岛构建：hydrated 图表走 WASM + asgard glue（模板来自 VOA 工程 AWSL）。
use crate::{
    asgard::error::{AsgardError, AsgardResult},
    legion::CanonicalTarget,
};
use nyar_analyzer::report::{HydratedChartSpec, series_to_init_literal};
use oak_awsl::widget_name_from_stem;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::asgard::{
    awsl::{LoweringOptions, compile_awsl_source},
    codegen::{IslandPackageOptions, asgard_boot_script_tag, asgard_boot_stylesheet_tag, build_awsl_wasm_source, package_browser_islands},
    compile::{GeneratedSource, dependency_keys_for_packages, resolve_project_source_groups},
    ssg::project::read_project_source,
    wasm::{compile_wasm_bundle, copy_wasm_artifacts_to_dist},
};

/// 构建 hydrated 图表岛，返回 `(route, mount_html)`。
///
/// `roots` 是调用方已解析的源码库根目录（workspace 事实或显式配置），
/// Asgard 不再自行推断。
pub fn emit_report_chart_islands(
    project_dir: &Path,
    output_dir: &Path,
    module_stem: &str,
    charts: &[HydratedChartSpec],
    roots: &[PathBuf],
) -> AsgardResult<Vec<(String, String)>> {
    if charts.is_empty() {
        return Ok(Vec::new());
    }

    let icp_awsl = load_interactive_col_plot(roots)?;

    fs::create_dir_all(output_dir)?;

    let mut components = Vec::new();
    let mut slots = Vec::new();
    // 注入的 `series` 字面量必须是具名结构字面量：元素类型逐字取自 wrapper 的
    // `let series: [package::Type]` 声明，其所属包随后按名挂接为生成组的显式依赖。
    let mut series_packages = std::collections::BTreeSet::new();

    for chart in charts {
        let wrapper = read_project_source(project_dir, &format!("charts/{}.awsl", chart.route))?;
        // 生成期的唯一元素类型来源是 wrapper 的 `let series: [package::Type]` 声明；
        // 声明缺失、形态不符或类型路径非法时闭合失败，不由生成器猜类型名。
        let (_, series_declaration) = extract_title_series_lets(&wrapper)?;
        let series_element = parse_series_element_type(&series_declaration, &chart.route)?;
        series_packages.insert(series_element.package);
        let title_lit = encode_v_text_literal(&chart.title)
            .map_err(|error| AsgardError::msg(report!("chart {}: {error}", chart.route)))?;
        let series_lit = series_to_init_literal(&chart.series, &series_element.path)
            .map_err(|error| AsgardError::msg(report!("chart {}: {error}", chart.route)))?;
        let filled = fill_chart_placeholders(&wrapper, &title_lit, &series_lit);
        let awsl = expand_interactive_col_plot(&filled, &icp_awsl, &chart.route)?;
        let component = compile_awsl_source(&awsl, &chart.route, &format!("{}.awsl", chart.route), &LoweringOptions::default())
            .map_err(|error| AsgardError::msg(report!("compile chart AWSL {}: {error}", chart.route)))?;
        let mount = format!(
            r#"<div data-island="hydrated" data-component="{route}" data-module="{module}"></div>"#,
            route = chart.route,
            module = module_stem,
        );
        slots.push((chart.route.clone(), mount));
        components.push(component);
    }

    if components.is_empty() {
        return Ok(Vec::new());
    }

    let wasm_stem = module_stem.replace('.', "-");
    let awsl_v = build_awsl_wasm_source(&components)?;
    let resolved = resolve_project_source_groups(project_dir, &CanonicalTarget::browser())?;
    // 生成代码引用 `usize` 别名与 `Array` 合同（`<loop>` 的条目数量与下标）：
    // 按包名显式挂接 `std`，不泛化挂接整个 workspace 或整个 std 快照。
    let mut extra_dependency_keys = dependency_keys_for_packages(&resolved.groups, &["std"]);
    // 具名结构字面量在生成组里构造 `series` 条目，类型所属包必须是生成组的显式依赖，
    // 否则名称解析必然失败。包缺席时闭合失败，不回退到匿名字面量。
    for package in &series_packages {
        let keys = dependency_keys_for_packages(&resolved.groups, &[package.as_str()]);
        if keys.len() != 1 {
            return Err(AsgardError::msg(report!(
                "chart `series` element type belongs to package `{package}`, which is not in the project dependency closure"
            )));
        }
        extra_dependency_keys.extend(keys);
    }
    let generated = GeneratedSource { producer: "asgard.awsl.report-island", source: &awsl_v };
    // 报告图表岛走非入口执行路径：岛包源码组只作为依赖，入口身份唯一来自生成组的显式 `[export]`。
    let wasm_built = match compile_wasm_bundle(
        &resolved.groups,
        generated,
        output_dir,
        module_stem,
        &CanonicalTarget::browser(),
        crate::asgard::compile::GeneratedEntrySurface::GeneratedOnly,
        &extra_dependency_keys,
        &resolved.host_provider_bindings,
        &resolved.gated_out_host_providers,
    ) {
        Ok(report) => {
            copy_wasm_artifacts_to_dist(output_dir, &report)
                .map_err(|e| AsgardError::msg(format!("{e}: {}", "报告图表岛 WASM 产物复制失败")))?;
            true
        }
        Err(error) => {
            // 图表岛按各图表 `let series` 声明的元素类型编译；元素类型所属包必须
            // 在当前 target 的依赖闭包内。包缺席时的原始诊断只会指向下游的
            // 名字解析失败（例如 `infix ==` 无候选），这里补上缺席包与闭包清单，
            // 让人能直接判断是闭包缺口而不是别处坏了。
            let closure = resolved.groups.iter().map(|group| group.name.as_str()).collect::<Vec<_>>().join(", ");
            let missing = series_packages
                .iter()
                .filter(|package| dependency_keys_for_packages(&resolved.groups, &[package.as_str()]).is_empty())
                .map(String::as_str)
                .collect::<Vec<_>>();
            let closure_note = if missing.is_empty() {
                String::new()
            } else {
                format!(
                    "（图表岛闭包缺少 series 元素类型所属包 `{}`；当前依赖闭包为：{closure}。\
                     需要该包支持当前 target，或把契约类型移入可构建包）",
                    missing.join("`、`")
                )
            };
            Err(AsgardError::msg(format!("报告图表岛 WASM 编译失败{closure_note}: {error}")))?
        }
    };

    let opts = IslandPackageOptions {
        module_stem: module_stem.to_string(),
        wasm_stem,
        wasm_built,
        relative_urls: true,
        css_mode: "merge".into(),
        mode: "report".into(),
        tailwind_css: None,
        porting_cost: Vec::new(),
    };
    package_browser_islands(&components, output_dir, &opts).map_err(|e| AsgardError::msg(format!("{e}: {}", "package chart islands")))?;

    Ok(slots)
}

/// Head tags for asgard boot CSS (markup only).
pub fn asgard_boot_head_tags(css_name: &str) -> String {
    asgard_boot_stylesheet_tag(css_name)
}

/// Boot script tag before `</body>`（仅 `src=`，start 在 auto boot.js 内）。
pub fn asgard_start_snippet() -> &'static str {
    asgard_boot_script_tag(true)
}

fn load_interactive_col_plot(roots: &[std::path::PathBuf]) -> AsgardResult<String> {
    let rel = "source/components/interactive-col-plot.awsl";
    let candidates = ["projects/asgard.plotter", "projects/.asgard/projects/asgard.plotter", "projects/asgard._/projects/asgard.plotter"];
    for root in roots {
        for project in candidates {
            let path = root.join(project).join(rel);
            if path.is_file() {
                return fs::read_to_string(&path).map_err(|e| AsgardError::msg(format!("read {}: {e}", path.display())));
            }
        }
    }
    Err(AsgardError::msg(report!("missing asgard.plotter InteractiveColPlot.awsl")))
}

fn fill_chart_placeholders(wrapper: &str, title_lit: &str, series_lit: &str) -> String {
    wrapper.replace("__TITLE__", title_lit).replace("__SERIES__", series_lit)
}

/// 把图表标题编码成可解析的 VX 字符串字面量。
///
/// VX 词法没有反斜杠转义（`\n`/`\u{..}`/`\"` 都不成立），字符串由引号 run 定界：
/// - `{` 开插值，必须写成 `\{`；`}` 原样输出；
/// - `\` 只在紧跟 `{`/`}` 时被剥离，此时双写；其余位置原样；
/// - 内容里的 `"` 由更大的引号 run 容纳（run 长度必须小于定界符个数）；
/// - 首/尾 `"` 无法在单个字面量里表示，而浏览器闭包里文本 `concat` 实例化不了
///   （`std.adaptor.wasm` 的文本 host provider 没有播种），因此这类标题闭合失败。
///
/// 其余无法还原的形态（除 `\n`/`\t` 外的控制字符）同样闭合失败，不生成语义漂移的字面量。
#[doc(hidden)]
pub fn encode_v_text_literal(text: &str) -> AsgardResult<String> {
    if let Some(ch) = text.chars().find(|ch| ch.is_control() && !matches!(ch, '\n' | '\t')) {
        return Err(AsgardError::msg(report!(
            "chart title contains control character U+{:04X}, which has no VX literal form",
            ch as u32
        )));
    }
    if text.starts_with('"') || text.ends_with('"') {
        return Err(AsgardError::msg(report!(
            "chart title starts or ends with `\"`, which VX cannot represent in a single literal and browser-closure text concat is not instantiable"
        )));
    }
    let longest_quote_run = text
        .chars()
        .fold((0usize, 0usize), |(longest, current), ch| {
            if ch == '"' {
                let next = current + 1;
                (longest.max(next), next)
            } else {
                (longest, 0)
            }
        })
        .0;
    let quote_count = if longest_quote_run == 0 { 1 } else { (longest_quote_run + 1).max(3) };
    let delimiter = "\"".repeat(quote_count);
    Ok(format!("{delimiter}{}{delimiter}", encode_v_text_body(text)))
}

/// 只编码实测可还原的转义：`{` → `\{`，`\` 紧跟 `{`/`}` 时双写，其余原样。
fn encode_v_text_body(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => out.push_str("\\{"),
            '\\' => match chars.peek() {
                Some('{') | Some('}') => {
                    out.push_str("\\\\");
                    out.push(chars.next().expect("peeked character must be present"));
                }
                _ => out.push('\\'),
            },
            _ => out.push(ch),
        }
    }
    out
}

fn expand_interactive_col_plot(wrapper: &str, icp_awsl: &str, route: &str) -> AsgardResult<String> {
    let (title_line, series_line) = extract_title_series_lets(wrapper)?;
    // `series` 声明逐字来自 wrapper，元素类型已由 `parse_series_element_type` 闭合校验；
    // 生成器不再改写声明，注入的字面量与声明的元素类型一一对应。
    let widget_name = widget_name_from_stem(route);
    let mut out = icp_awsl.replacen("<widget interactive_col_plot>", &format!("<widget {widget_name}>"), 1);
    if !out.contains(&format!("<widget {widget_name}>")) {
        return Err(AsgardError::msg(report!("InteractiveColPlot template missing widget tag")));
    }
    out = replace_let_line(&out, "title", &title_line)?;
    out = replace_let_line(&out, "series", &series_line)?;
    Ok(out)
}

/// `series` 声明携带的元素类型。
struct SeriesElementType {
    /// 声明里逐字写出的类型路径，作为生成结构字面量的构造名。
    path: String,
    /// 类型所属包名（`path` 最后一个 `::` 之前的部分），用于按名挂接生成组依赖。
    package: String,
}

/// 从 wrapper 的 `let series: [package::Type] = …` 声明里取出元素类型。
///
/// 生成期的唯一类型来源就是这条声明：`ColSeriesItem` 自身不携带 VX 身份，ABI 派生绑定
/// 也没有元素类型。声明缺失、未写类型、不是 `[T]` 形态或不是包限定路径时闭合失败，
/// 不允许生成器自行猜一个类型名。
fn parse_series_element_type(declaration: &str, route: &str) -> AsgardResult<SeriesElementType> {
    let rest = declaration
        .trim()
        .strip_prefix("let series")
        .ok_or_else(|| AsgardError::msg(report!("chart {route}: `let series` declaration missing")))?
        .trim_start();
    let rest = rest
        .strip_prefix(':')
        .ok_or_else(|| AsgardError::msg(report!("chart {route}: `let series` declaration must carry an element type `[package::Type]`")))?
        .trim_start();
    let array = rest
        .strip_prefix('[')
        .ok_or_else(|| AsgardError::msg(report!("chart {route}: `let series` declaration must use array element type `[T]`, found `{rest}`")))?;
    let close = array
        .find(']')
        .ok_or_else(|| AsgardError::msg(report!("chart {route}: `let series` array element type is not closed")))?;
    let element = array[..close].trim();
    let tail = array[close + 1..].trim_start();
    if !(tail.is_empty() || tail.starts_with('=')) {
        return Err(AsgardError::msg(report!("chart {route}: `let series` declaration has trailing text after the element type: `{tail}`")));
    }
    let Some((package, name)) = element.rsplit_once("::")
    else {
        return Err(AsgardError::msg(report!(
            "chart {route}: `let series` element type `{element}` must be package-qualified (`package::Type`) because the generated island group is a separate package"
        )));
    };
    if !is_package_path(package) || !is_type_identifier(name) {
        return Err(AsgardError::msg(report!("chart {route}: `let series` element type `{element}` is not a valid package-qualified type path")));
    }
    Ok(SeriesElementType { path: element.to_string(), package: package.to_string() })
}

fn is_package_path(path: &str) -> bool {
    !path.is_empty() && path.split('.').all(is_type_identifier)
}

fn is_type_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next()
    else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_') && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn extract_title_series_lets(wrapper: &str) -> AsgardResult<(String, String)> {
    let mut title = None;
    let mut series = None;
    for line in wrapper.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("let title") {
            title = Some(trimmed.to_string());
        }
        else if trimmed.starts_with("let series") {
            series = Some(trimmed.to_string());
        }
    }
    match (title, series) {
        (Some(t), Some(s)) => Ok((t, s)),
        _ => Err(AsgardError::msg(report!("chart wrapper missing let title / let series"))),
    }
}

fn replace_let_line(source: &str, name: &str, replacement: &str) -> AsgardResult<String> {
    let prefix = format!("let {name}");
    let mut found = false;
    let mut out = String::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            out.push_str("    ");
            out.push_str(replacement);
            out.push('\n');
            found = true;
        }
        else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !found {
        return Err(AsgardError::msg(report!("InteractiveColPlot missing `{prefix}` binding")));
    }
    Ok(out)
}
