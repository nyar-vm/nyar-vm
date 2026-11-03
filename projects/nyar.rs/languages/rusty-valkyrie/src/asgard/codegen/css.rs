//! CSS 提取与切分：`merge` 输出单一全局样式文件；`per-component` 按组件切分为
//! 独立 css chunk（命名合同为 [`crate::asgard::package::component_css_filename`]）。

use crate::asgard::{awsl::LoweredComponent, package::component_css_filename};

/// CSS 输出。
#[derive(Debug, Clone)]
pub struct CssOutput {
    /// 合并后的 CSS 文本。
    pub content: String,
    /// 输出文件名（不含目录）。
    pub filename: String,
}

/// 从组件列表提取样式并按模式切分。
pub fn extract_and_merge_styles(
    components: &[LoweredComponent],
    project_name: &str,
    css_mode: &str,
    tailwind_css: Option<&str>,
) -> Vec<CssOutput> {
    match css_mode {
        "per-component" => {
            let mut outputs = components
                .iter()
                .map(|component| CssOutput { content: crate::asgard::style::web::component_web_css(component), filename: component_css_filename(&component.name) })
                .filter(|output| !output.content.trim().is_empty())
                .collect::<Vec<_>>();
            if let Some(extra) = tailwind_css {
                if let Some(first) = outputs.first_mut() {
                    crate::asgard::codegen::tailwind::append_tailwind_css(&mut first.content, extra);
                }
                else {
                    outputs.push(CssOutput { content: format!("/* tailwind */\n{extra}"), filename: format!("{project_name}.css") });
                }
            }
            outputs
        }
        _ => {
            let mut merged = String::new();
            for component in components {
                let css = crate::asgard::style::web::component_web_css(component);
                if !css.trim().is_empty() {
                    merged.push_str(&format!("/* {} */\n", component.name));
                    merged.push_str(&css);
                    merged.push('\n');
                }
            }
            if let Some(extra) = tailwind_css {
                crate::asgard::codegen::tailwind::append_tailwind_css(&mut merged, extra);
            }
            vec![CssOutput { content: merged, filename: format!("{project_name}.css") }]
        }
    }
}
