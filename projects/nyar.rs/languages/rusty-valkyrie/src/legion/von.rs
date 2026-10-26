//! VON facade — 直连 [`oak_von`]，不经 `vcc-data::text::von`。

use std::result::Result;

/// VON 解析/序列化错误类型，复用 `oak_core::OakError`。
pub type VonError = oak_core::OakError;
pub use oak_von::{language::value::VonValue, to_string, to_string_indented};

/// 解析 VON 文本为 [`VonValue`]（兼容旧 `VonParser::parse` 调用点）。
pub struct VonParser;

impl VonParser {
    /// 解析 VON 文本为 [`VonValue`]。
    pub fn parse(source: &str) -> Result<VonValue, VonError> {
        let trimmed = strip_leading_trivia(source);
        let ast = oak_von::parse(trimmed).map_err(VonError::custom_error)?;
        Ok(oak_von::language::value::from_ast(&ast))
    }
}

/// 将 VON 文本反序列化为 Rust 类型。
///
/// `oak_von` 的 AST builder 以根节点首个子节点的字节偏移 0 为基准读取字段，
/// 前导空白/`#` 行注释会把字段 span 整体前移，导致字段名与值错位。进入
/// `oak_von` 前剥离前导 trivia，保证第一个结构从字节 0 开始。
pub fn from_str<T>(source: &str) -> Result<T, VonError>
where
    T: serde::de::DeserializeOwned,
{
    oak_von::from_str(strip_leading_trivia(source))
}

/// 剥离前导空白与整行 `#` 注释；其余内容保持原样。
fn strip_leading_trivia(source: &str) -> &str {
    let mut rest = source;
    loop {
        rest = rest.trim_start();
        let Some(comment) = rest.strip_prefix('#')
        else {
            return rest;
        };
        let end = comment.find(['\n', '\r']).unwrap_or(comment.len());
        rest = &comment[end..];
    }
}

/// 将 [`VonValue`] 反序列化为 Rust 类型。
pub fn from_value<T>(value: VonValue) -> Result<T, VonError>
where
    T: serde::de::DeserializeOwned,
{
    let text = oak_von::printer::print_value(&value, &oak_von::printer::PrintOptions::compact());
    oak_von::from_str(&text)
}
