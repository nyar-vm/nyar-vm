//! 将报告数据序列化为 AWSL script 可注入的字面量。

use super::spec::ColSeriesItem;

/// 图表 series 元素在 V 侧的契约类型（`asgard.plotter` 的 `ColSeriesItem`）。
///
/// 注入脚本在全局命名空间生成，元素必须带完整构造式类型名，否则 VX 解析只能
/// 看到裸 `{ ... }`（不是 primary），词法失败后整个脚本 ABI 提取为空。
pub const SERIES_ITEM_V_TYPE: &str = "asgard.plotter::ColSeriesItem";

/// 将 series 序列化为带元素类型的构造式字面量（用于 `let series: list = …`）。
///
/// 容器本身由注入点声明；每个元素显式构造 `ColSeriesItem { ... }`，让后续
/// `series[index]` / `item.field` 的类型在 V 侧可判定。
pub fn series_to_init_literal(series: &[ColSeriesItem]) -> String {
    if series.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = series.iter().map(item_to_literal).collect();
    format!("[{}]", items.join(", "))
}

fn item_to_literal(item: &ColSeriesItem) -> String {
    format!(
        "{ty} {{ key: {}, label: {}, value_text: {}, fill: {}, height_pct: {} }}",
        vx_text_literal(&item.key),
        vx_text_literal(&item.label),
        vx_text_literal(&item.value_text),
        vx_text_literal(&item.fill),
        f64_literal(item.height_pct),
        ty = SERIES_ITEM_V_TYPE,
    )
}

/// VX 字符串字面量编码。
///
/// VX 词法没有反斜杠转义：`\n` / `\"` / `\u{..}` 都不成立。字符串由引号 run 定界，
/// 内容里的 `"` 由更大的引号 run 容纳；`{` 开插值，必须写成 `\{`；`\` 紧跟
/// `{` / `}` 时双写，其余位置原样。
fn vx_text_literal(text: &str) -> String {
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
    let delimiter = "\"".repeat(if longest_quote_run == 0 { 1 } else { (longest_quote_run + 1).max(3) });
    let mut body = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => body.push_str("\\{"),
            '\\' => match chars.peek() {
                Some('{') | Some('}') => {
                    body.push_str("\\\\");
                    body.push(chars.next().expect("peeked character must be present"));
                }
                _ => body.push('\\'),
            },
            _ => body.push(ch),
        }
    }
    format!("{delimiter}{body}{delimiter}")
}

fn f64_literal(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 { format!("{value:.1}") } else { format!("{value}") }
}
