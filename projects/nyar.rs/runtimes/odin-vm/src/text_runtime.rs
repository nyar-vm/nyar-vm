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

/// 原样拼接：不做任何编码转换或规范化，两侧内容逐字节保留。
pub fn concat(left: &str, right: &str) -> String {
    let mut result = String::with_capacity(left.len() + right.len());
    result.push_str(left);
    result.push_str(right);
    result
}

/// 按码点序列相等判定；不做 Unicode 规范化（组合式与预组合式不等）。
///
/// 在合法 UTF-8 上等价于字节相等，但按码点走可避免未来引入规范化实现时语义漂移。
pub fn equals(left: &str, right: &str) -> bool {
    left.chars().eq(right.chars())
}

/// UTF-8 → UTF-16 码元序列：astral 码点产出代理对（U+1F600 → `D83D DE00`）。
pub fn to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// UTF-16 码元计数：代理对算 2 个单元，不是码点数。
pub fn utf16_length(units: &[u16]) -> i32 {
    units.len() as i32
}

/// UTF-16 → UTF-8：孤立代理（`D800..=DFFF` 落单）结构化失败，不替换、不丢弃。
pub fn from_utf16(units: &[u16], operation: &'static str) -> Result<String, OdinRuntimeError> {
    let mut text = String::with_capacity(units.len());
    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(value) => text.push(value),
            Err(_) => return Err(OdinRuntimeError::InvalidUtf8 { operation }),
        }
    }
    Ok(text)
}

/// 第 `char_index` 个码点的字节偏移；越界时返回字节末尾（仅内部使用，调用方已判界）。
fn byte_offset(text: &str, char_index: usize) -> usize {
    text.char_indices().nth(char_index).map(|(offset, _)| offset).unwrap_or(text.len())
}
