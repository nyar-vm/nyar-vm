//! 将报告数据序列化为 AWSL script 可注入的字面量。

use super::spec::ColSeriesItem;

/// [`series_to_init_literal`] 的失败原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLiteralError {
    /// 调用方无法给出 `series` 的元素类型，或给出的不是合法 VX 类型路径。
    MissingElementType,
}

impl std::fmt::Display for ReportLiteralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingElementType => write!(f, "无法为 `series` 确定元素类型"),
        }
    }
}

impl std::error::Error for ReportLiteralError {}

/// 将 series 序列化为具名结构字面量列表（用于 `let series: [T] = …`）。
///
/// `element_type` 是 `series` 声明里逐字写出的元素类型路径，由持有该声明的调用方提供。
/// 本函数不推导、也不猜测类型名：缺失或不是合法类型路径时失败闭合，绝不退化成匿名结构
/// 字面量——匿名结构字面量会让 oak-vx 的 ABI 提取整体失败，进而丢掉 `series` 绑定。
pub fn series_to_init_literal(series: &[ColSeriesItem], element_type: &str) -> Result<String, ReportLiteralError> {
    if !is_vx_type_path(element_type) {
        return Err(ReportLiteralError::MissingElementType);
    }
    if series.is_empty() {
        return Ok("[]".to_string());
    }
    let items: Vec<String> = series.iter().map(|item| item_to_literal(item, element_type)).collect();
    Ok(format!("[{}]", items.join(", ")))
}

fn item_to_literal(item: &ColSeriesItem, element_type: &str) -> String {
    format!(
        "{element_type} {{ key: {}, label: {}, value_text: {}, fill: {}, height_pct: {} }}",
        vx_text_literal(&item.key),
        vx_text_literal(&item.label),
        vx_text_literal(&item.value_text),
        vx_text_literal(&item.fill),
        f64_literal(item.height_pct),
    )
}

/// 接受 `Type`、`package::Type` 与 `package.path::Type` 形态的类型路径。
fn is_vx_type_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return false;
    }
    path.split("::").all(is_dotted_identifier_path)
}

fn is_dotted_identifier_path(part: &str) -> bool {
    !part.is_empty() && part.split('.').all(is_vx_identifier)
}

fn is_vx_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next()
    else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
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
