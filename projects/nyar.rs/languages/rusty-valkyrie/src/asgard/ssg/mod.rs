//! 静态站点生成（SSG）：AWSL → 纯 HTML 页面，无 hydration。

pub mod project;

use std::fmt::Write as _;

use serde_json::Value;

use crate::asgard::{
    awsl::{LoweringOptions, compile_awsl_source},
    codegen::html_render::{StaticRenderResult, render_static_ir},
    error::AsgardResult,
};

/// 将页面 HTML 注入 layout 的 `<slot />` 占位符。
pub fn wrap_with_layout(layout_source: &str, body_html: &str) -> String {
    layout_source.replace("<slot />", body_html).replace("<slot/>", body_html).replace("<slot></slot>", body_html)
}

/// 解析 AWSL 源码并静态渲染。
///
/// 编译错误（含 ABI 与脚本不一致）向上传播，不在 SSG 里把失败转成 panic。
pub fn render_awsl_static(source: &str, component_name: &str, source_path: &str, data: &Value) -> AsgardResult<StaticRenderResult> {
    let component = compile_awsl_source(source, component_name, source_path, &LoweringOptions::default())?;
    Ok(render_static_ir(&component, data))
}

/// 生成完整静态 HTML 页面（无 `asgard-runtime.js`）。
pub fn generate_static_page(title: &str, results: &[StaticRenderResult], extra_css: &str) -> String {
    generate_static_page_with_base(title, results, extra_css, "/")
}

/// 生成完整静态 HTML 页面，并把 `base`（配置拥有的目录 URL）写入 `<base href>`。
///
/// 缺省基址 `/` 不输出 `<base>`，保持根部署页面与既有产物一致；非根基址用于子目录部署，
/// 让页面内相对引用按配置基址解析。`base` 的规范化与合法性由配置所有者负责，
/// 本函数只消费已解析的目录 URL。
pub fn generate_static_page_with_base(title: &str, results: &[StaticRenderResult], extra_css: &str, base: &str) -> String {
    generate_static_page_full(title, results, extra_css, base, "", "")
}

/// 生成完整静态 HTML 页面，并在 `</head>` / `</body>` 之前追加调用方给出的标记片段。
///
/// `head_tags` 用于样式表等头部引用，`body_tags` 用于 `boot.js` 等启动脚本引用；
/// 两段都是已由产物所有者生成的标记，本函数只负责插入位置与缩进，不解释其内容。
pub fn generate_static_page_full(
    title: &str,
    results: &[StaticRenderResult],
    extra_css: &str,
    base: &str,
    head_tags: &str,
    body_tags: &str,
) -> String {
    let mut css = String::new();
    if !extra_css.trim().is_empty() {
        css.push_str(extra_css.trim());
        css.push('\n');
    }
    for result in results {
        if !result.css.trim().is_empty() {
            css.push_str(&format!("/* {} */\n", result.scope));
            css.push_str(result.css.trim());
            css.push('\n');
        }
    }

    let mut body = String::new();
    for result in results {
        body.push_str(&result.html);
        body.push('\n');
    }

    let mut page = String::new();
    writeln!(page, "<!DOCTYPE html>").unwrap();
    writeln!(page, "<html lang=\"zh-CN\">").unwrap();
    writeln!(page, "<head>").unwrap();
    writeln!(page, "  <meta charset=\"utf-8\">").unwrap();
    writeln!(page, "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">").unwrap();
    writeln!(page, "  <title>{}</title>", escape_html(title)).unwrap();
    let base = base.trim();
    if base != "/" {
        writeln!(page, "  <base href=\"{}\">", escape_html(base)).unwrap();
    }
    if !head_tags.trim().is_empty() {
        write!(page, "{}", head_tags.trim_end()).unwrap();
        page.push('\n');
    }
    if !css.trim().is_empty() {
        writeln!(page, "  <style>").unwrap();
        for line in css.lines() {
            writeln!(page, "    {line}").unwrap();
        }
        writeln!(page, "  </style>").unwrap();
    }
    writeln!(page, "</head>").unwrap();
    writeln!(page, "<body>").unwrap();
    writeln!(page, "{}", body.trim()).unwrap();
    if !body_tags.trim().is_empty() {
        writeln!(page, "{}", body_tags.trim()).unwrap();
    }
    writeln!(page, "</body>").unwrap();
    writeln!(page, "</html>").unwrap();
    page
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
