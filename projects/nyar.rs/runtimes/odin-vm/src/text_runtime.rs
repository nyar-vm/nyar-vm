//! 文本编码运行时的纯语义层：UTF-8 / UTF-16 / C 字符串的长度、切片与转换。
//!
//! 索引空间合同：
//! - UTF-8 的 `length_chars` 与 `slice` 一律按 **Unicode 码点**（scalar）计，不按字节；
//! - `slice(start, end)` 是半开区间 `[start, end)`，越界 **结构化失败**，绝不 clamp；
//! - 非法编码一律 fail-closed（非法 UTF-8 / 孤立 UTF-16 代理），不产生 U+FFFD 替换；
//! - C 字符串按首个 NUL 终止，缺 NUL 或被转文本含内嵌 NUL 均拒绝。

use crate::error::OdinRuntimeError;

/// UTF-8 字节长度。
pub fn length_bytes(text: &str) -> i32 {
    text.len() as i32
}

/// Unicode 码点计数（astral 字符算 1，组合附加符按各自码点计数）。
pub fn length_chars(text: &str) -> i32 {
    text.chars().count() as i32
}

/// 按码点下标取半开区间 `[start, end)`；任何越界（含 `start > end`、负下标）结构化失败。
pub fn slice(text: &str, start: i64, end: i64, operation: &'static str) -> Result<String, OdinRuntimeError> {
    let length = length_chars(text);
    let length_64 = length as i64;
    if start < 0 || end < 0 || start > length_64 || end > length_64 || start > end {
        return Err(OdinRuntimeError::IndexOutOfBounds { operation, start, end, length: length as u64 });
    }
    let start_byte = byte_offset(text, start as usize);
    let end_byte = byte_offset(text, end as usize);
    Ok(text[start_byte..end_byte].to_string())
}

/// 第 `char_index` 个码点的字节偏移；越界时返回字节末尾（仅内部使用，调用方已判界）。
fn byte_offset(text: &str, char_index: usize) -> usize {
    text.char_indices().nth(char_index).map(|(offset, _)| offset).unwrap_or(text.len())
}
