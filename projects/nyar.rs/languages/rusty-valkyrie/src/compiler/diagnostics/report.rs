//! 多诊断报告的确定性文本渲染（rustc 风格）。
//!
//! 输入是诊断暂存汇（[`PendingDiagnostics`]）取出的全部诊断，输出是一段稳定文本：
//! 按 `(文件, span 起点, span 终点, code)` 排序后逐条渲染，
//! 每条带 `error[code]: message`、`--> file:line:col` 定位行、源码片段与 `^` 指向。
//! 同一输入渲染两次必须得到完全一致的输出（排序只依赖诊断自身字段）。

use nyar_types::DiagnosticSpan;

use super::pending::PendingDiagnostics;

/// 渲染配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticRenderOptions {
    /// 单个编译单元最多渲染的诊断条数；超出的部分以汇总行说明，不静默截断。
    pub max_diagnostics: usize,
}

impl Default for DiagnosticRenderOptions {
    fn default() -> Self {
        Self { max_diagnostics: 50 }
    }
}

impl DiagnosticRenderOptions {
    /// 从环境变量 `VALKYRIE_DIAGNOSTIC_LIMIT` 读取展示上限；未设置或非法时回落到默认值。
    pub fn from_environment() -> Self {
        let limit = std::env::var("VALKYRIE_DIAGNOSTIC_LIMIT").ok().and_then(|raw| raw.parse::<usize>().ok()).filter(|limit| *limit > 0);
        match limit {
            Some(max_diagnostics) => Self { max_diagnostics },
            None => Self::default(),
        }
    }
}

/// 渲染失败路径上收集到的全部诊断。
///
/// `resolve_source` 按诊断的 `source.id` 提供源文件原文；拿不到原文时退化为字节区间定位行，
/// 不伪造源码片段。空输入渲染为空串。
pub fn render_compile_diagnostics(
    pending: &PendingDiagnostics,
    resolve_source: &dyn Fn(&str) -> Option<String>,
    options: &DiagnosticRenderOptions,
) -> String {
    if pending.is_empty() {
        return String::new();
    }
    let mut order: Vec<usize> = (0..pending.diagnostics.len()).collect();
    order.sort_by(|left, right| {
        sort_key(
            &pending.diagnostics[*left].row.source.id,
            &pending.diagnostics[*left].row.primary_span,
            &pending.diagnostics[*left].row.code,
            &pending.diagnostics[*left].message,
        )
        .cmp(&sort_key(
            &pending.diagnostics[*right].row.source.id,
            &pending.diagnostics[*right].row.primary_span,
            &pending.diagnostics[*right].row.code,
            &pending.diagnostics[*right].message,
        ))
    });

    let mut rendered = String::new();
    for index in order.iter().take(options.max_diagnostics) {
        let diagnostic = &pending.diagnostics[*index];
        rendered.push_str(&render_one(
            &diagnostic.message,
            &diagnostic.row.code,
            &diagnostic.row.source.id,
            &diagnostic.row.primary_span,
            resolve_source,
        ));
        rendered.push('\n');
    }
    let not_shown = order.len().saturating_sub(options.max_diagnostics) + pending.overflow;
    if not_shown > 0 {
        rendered.push_str(&format!("note: {} more diagnostic(s) not shown (limit {} reached)\n", not_shown, options.max_diagnostics));
    }
    let total = pending.total();
    rendered.push_str(&format!("error: aborting due to {} previous error{}\n", total, plural_suffix(total)));
    rendered
}

/// 确定性排序键：先文件，再 span 起点、终点，再 code 与消息。
fn sort_key<'a>(source: &'a str, span: &'a DiagnosticSpan, code: &'a str, message: &'a str) -> (&'a str, usize, usize, &'a str, &'a str) {
    (source, span.start, span.end, code, message)
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// 渲染单条诊断块（消息行 + 定位行 + 可选源码片段）。
fn render_one(message: &str, code: &str, source_id: &str, span: &DiagnosticSpan, resolve_source: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = format!("error[{code}]: {message}\n");
    let source_text = resolve_source(source_id);
    match source_text.as_deref().map(|text| locate(text, span)) {
        Some(located) => {
            out.push_str(&format!("  --> {source_id}:{}:{}\n", located.line, located.column));
            let width = located.line.to_string().len();
            out.push_str(&format!("{} |\n", " ".repeat(width)));
            out.push_str(&format!("{} | {}\n", located.line, located.line_text));
            out.push_str(&format!("{} | {}{}\n", " ".repeat(width), " ".repeat(located.column - 1), "^".repeat(located.caret_len)));
        }
        None => {
            out.push_str(&format!("  --> {source_id}:{}..{}\n", span.start, span.end));
        }
    }
    out
}

/// 由字节 span 计算行列定位与指向宽度。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Located<'a> {
    /// 1-based 行号。
    line: usize,
    /// 1-based 列号（按字符计）。
    column: usize,
    /// 所在行原文（不含换行）。
    line_text: &'a str,
    /// `^` 指向宽度（字符数，至少 1，跨行截断到行尾）。
    caret_len: usize,
}

/// 把字节 span 映射到源码行；span 越界时裁剪到文本范围，落不到字符边界时对齐到前一个边界。
fn locate<'a>(text: &'a str, span: &DiagnosticSpan) -> Located<'a> {
    let mut start = span.start.min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = span.end.min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    let end = end.max(start);
    let line_start = text[..start].rfind('\n').map(|index| index + 1).unwrap_or(0);
    let line = text[..line_start].bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_end = text[start..].find('\n').map(|index| start + index).unwrap_or(text.len());
    let line_text = &text[line_start..line_end];
    let column = text[line_start..start].chars().count() + 1;
    let caret_len = text[start..end.min(line_end)].chars().count().max(1);
    Located { line, column, line_text, caret_len }
}
