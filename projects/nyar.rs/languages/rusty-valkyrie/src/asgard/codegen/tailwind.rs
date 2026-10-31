//! Tailwind CLI 产出的 CSS 合并。
//!
//! Tailwind CLI 构建入口随旧文本转换配置链清退；配置求值（C0/C2）闭合后由结构化配置重新接入。

/// 将 Tailwind 产出追加到合并 CSS 文本末尾。
pub fn append_tailwind_css(merged_css: &mut String, tailwind_css: &str) {
    if tailwind_css.trim().is_empty() {
        return;
    }
    if !merged_css.is_empty() && !merged_css.ends_with('\n') {
        merged_css.push('\n');
    }
    merged_css.push_str("/* tailwind */\n");
    merged_css.push_str(tailwind_css);
    if !merged_css.ends_with('\n') {
        merged_css.push('\n');
    }
}
