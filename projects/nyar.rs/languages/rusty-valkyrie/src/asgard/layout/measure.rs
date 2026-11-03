//! 文本度量：行盒计算由布局层自持，字体只是度量后端。
//!
//! 度量结果是布局层的输入：`break_lines` 产出行盒，尺寸约束（`Content`）
//! 由行盒 intrinsic size 决定。任何后端都不得把行盒计算交给宿主（CSS 的
//! 行盒、平台文本控件的自动换行都只是各自 lowering 的近似实现）。

use super::Points;

/// 字体身份：度量后端的寻址键，不是字体文件名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontId(pub String);

/// 一次待度量的文本运行。
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    /// 待度量文本。
    pub text: String,
    /// 度量后端寻址键。
    pub font: FontId,
    /// 字号（point）。
    pub size: Points,
}

/// 度量后端合同：字体层只提供 advance 与行高，不参与布局决策。
pub trait FontMetricsBackend {
    /// 整段运行的 advance 宽度（point）。
    fn advance(&self, run: &TextRun) -> Points;
    /// 行高（point）。
    fn line_height(&self, run: &TextRun) -> Points;
}

/// 一个行盒：布局层产物。
#[derive(Debug, Clone, PartialEq)]
pub struct LineBox {
    /// 行盒文本片段。
    pub text: String,
    /// 行盒宽度（point）。
    pub width: Points,
    /// 行盒高度（point）。
    pub height: Points,
}

/// 度量结果：行盒集合与 intrinsic 尺寸。
#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    /// 行盒集合（布局层产物）。
    pub lines: Vec<LineBox>,
    /// intrinsic 宽度（最宽行盒）。
    pub width: Points,
    /// intrinsic 高度（行高 × 行数）。
    pub height: Points,
}

/// 桩度量后端：CJK 表意文字按 1.0em、ASCII 按窄表、其余按 0.6em 估算。
///
/// 真实后端由各平台字体栈实现同一合同；桩只用于构建期静态度量与测试。
#[derive(Debug, Clone, Copy, Default)]
pub struct StubFontMetrics;

impl FontMetricsBackend for StubFontMetrics {
    fn advance(&self, run: &TextRun) -> Points {
        Points(run.text.chars().map(|ch| char_advance(ch) * run.size.0).sum())
    }

    fn line_height(&self, run: &TextRun) -> Points {
        Points(run.size.0 * 1.5)
    }
}

fn char_advance(ch: char) -> f32 {
    if is_cjk_ideograph(ch) {
        1.0
    }
    else if ch.is_ascii_whitespace() {
        0.3
    }
    else if ch.is_ascii_digit() || ch.is_ascii_uppercase() {
        0.62
    }
    else if ch.is_ascii() {
        0.52
    }
    else {
        0.6
    }
}

/// CJK 表意文字判定：中文度量单独验证的入口（全角 1.0em）。
pub fn is_cjk_ideograph(ch: char) -> bool {
    matches!(ch, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}')
}

/// 贪心断行：CJK 可在字间断行，拉丁在空格间断行；行盒宽度取实际 advance。
pub fn break_lines(run: &TextRun, max_width: Points, backend: &dyn FontMetricsBackend) -> TextMetrics {
    let line_height = backend.line_height(run);
    let mut lines: Vec<LineBox> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0.0f32;
    let mut widest = 0.0f32;

    let mut flush = |current: &mut String, width: &mut f32, lines: &mut Vec<LineBox>, line_height: Points| {
        let text = current.trim_end().to_string();
        if !text.is_empty() || lines.is_empty() {
            lines.push(LineBox { text, width: Points(*width), height: line_height });
        }
        *current = String::new();
        *width = 0.0;
    };

    let mut pending_space = false;
    for ch in run.text.chars() {
        let advance = char_advance(ch) * run.size.0;
        if ch.is_ascii_whitespace() {
            pending_space = true;
            continue;
        }
        let extra = if pending_space && !current.is_empty() { char_advance(' ') * run.size.0 } else { 0.0 };
        if current_width + extra + advance > max_width.0 && !current.is_empty() {
            widest = widest.max(current_width);
            flush(&mut current, &mut current_width, &mut lines, line_height);
            pending_space = false;
        }
        else if pending_space && !current.is_empty() {
            current.push(' ');
            current_width += extra;
            pending_space = false;
        }
        current.push(ch);
        current_width += advance;
    }
    widest = widest.max(current_width);
    flush(&mut current, &mut current_width, &mut lines, line_height);
    let line_count = lines.len().max(1);

    TextMetrics { lines, width: Points(widest), height: Points(line_height.0 * line_count as f32) }
}

/// 构建期度量入口：静态文本运行 + 度量后端 → 行盒与 intrinsic 尺寸。
pub fn measure_run(run: &TextRun, max_width: Points, backend: &dyn FontMetricsBackend) -> TextMetrics {
    break_lines(run, max_width, backend)
}
