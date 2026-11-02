//! 多诊断报告的确定性文本渲染（rustc 风格）。
//!
//! 输入是诊断暂存汇（[`PendingDiagnostics`]）取出的全部诊断，输出是一段稳定文本：
//! 按 `(文件, span 起点, span 终点, code)` 排序后逐条渲染，
//! 每条带 `error[code]: message`、`--> file:line:col` 定位行、源码片段与 `^` 指向。
//! 同一输入渲染两次必须得到完全一致的输出（排序只依赖诊断自身字段）。

use nyar_types::{DiagnosticArg, DiagnosticSpan};

use super::pending::{CompileDiagnostic, PendingDiagnostics};

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

    // 去级联：同一根因（code + 主标识参数）归并成一条，附受影响站点数；
    // 归并按根因键全局合并（首次出现的排序位置决定组的展示顺序），
    // 后续依赖同一失败事实的诊断不再各自刷屏。
    let diagnostics = &pending.diagnostics;
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_by_key = std::collections::BTreeMap::<String, usize>::new();
    for index in order {
        let key = root_cause_key(&diagnostics[index]);
        match group_by_key.get(&key) {
            Some(group_index) => groups[*group_index].push(index),
            None => {
                group_by_key.insert(key, groups.len());
                groups.push(vec![index]);
            }
        }
    }

    let mut rendered = String::new();
    let mut shown = 0usize;
    for group in groups.iter().take(options.max_diagnostics) {
        let first = &diagnostics[group[0]];
        rendered.push_str(&render_one(&first.message, &first.row.code, &first.row.source.id, &first.row.primary_span, resolve_source));
        if group.len() > 1 {
            let sites = group[1..].iter().take(5).map(|index| site_label(&diagnostics[*index], resolve_source)).collect::<Vec<_>>();
            let extra = group.len() - 1 - sites.len();
            let mut note =
                format!("   = {} more site{} with the same root cause: {}", group.len() - 1, plural_suffix(group.len() - 1), sites.join(", "));
            if extra > 0 {
                note.push_str(&format!(", +{extra} more"));
            }
            rendered.push_str(&note);
            rendered.push('\n');
        }
        rendered.push('\n');
        shown += group.len();
    }
    let not_shown = pending.total().saturating_sub(shown);
    if not_shown > 0 {
        rendered.push_str(&format!("note: {} more diagnostic(s) not shown (limit {} reached)\n", not_shown, options.max_diagnostics));
    }
    let total = pending.total();
    rendered.push_str(&format!("error: aborting due to {} previous error{}\n", total, plural_suffix(total)));
    rendered
}

/// 归并根因的稳定键：`code` 加主标识参数（`selector` / `name`），缺失时回退到消息文本。
fn root_cause_key(diagnostic: &CompileDiagnostic) -> String {
    let identity = diagnostic.row.args.get("selector").or_else(|| diagnostic.row.args.get("name"));
    match identity {
        Some(arg) => format!("{}|{}", diagnostic.row.code, render_arg(arg)),
        None => format!("{}|{}", diagnostic.row.code, diagnostic.message),
    }
}

fn render_arg(arg: &DiagnosticArg) -> String {
    match arg {
        DiagnosticArg::String(value)
        | DiagnosticArg::TypeName(value)
        | DiagnosticArg::Aggregate(value)
        | DiagnosticArg::CallableShort(value)
        | DiagnosticArg::DependencyKey(value) => value.clone(),
        DiagnosticArg::Int(value) => value.to_string(),
        DiagnosticArg::SymbolPath(parts) => parts.join("::"),
        DiagnosticArg::Span(span) => format!("{}..{}", span.start, span.end),
    }
}

/// 归并备注里的站点标签：能拿到源码时给 `file:line:col`，否则退化为字节区间。
fn site_label(diagnostic: &CompileDiagnostic, resolve_source: &dyn Fn(&str) -> Option<String>) -> String {
    let span = &diagnostic.row.primary_span;
    match resolve_source(&diagnostic.row.source.id).as_deref().map(|text| locate(text, span)) {
        Some(located) => format!("{}:{}:{}", diagnostic.row.source.id, located.line, located.column),
        None => format!("{}:{}..{}", diagnostic.row.source.id, span.start, span.end),
    }
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
