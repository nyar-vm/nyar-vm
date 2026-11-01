//! AWSL 文档解析辅助

use oak_awsl::{AwslRoot, parse_root};

/// 判断 URI 是否为 AWSL 文件。
///
/// 后缀集合取自单一事实源 [`nyar_format::formatter::AWSL_EXTENSIONS`]
/// （`.awsl` 规范拼写、`.w` 同义缩写，同 parser、同 lowering、同身份）。
pub fn is_awsl_uri(uri: &str) -> bool {
    nyar_format::formatter::AWSL_EXTENSIONS.iter().any(|extension| {
        let suffix = format!(".{extension}");
        uri.ends_with(&suffix) || uri.contains(&format!("{suffix}?"))
    })
}

/// 解析 AWSL 源文件
pub fn parse_awsl(text: &str) -> std::result::Result<AwslRoot, String> {
    parse_root(text).map_err(|error| error.message)
}

/// 若 `offset` 落在 `<script>` 块内，返回块范围及块内相对偏移
pub fn script_offset_at(root: &AwslRoot, source: &str, offset: usize) -> Option<(std::ops::Range<usize>, usize)> {
    let range = script_offset_range(root, source)?;
    if range.contains(&offset) { Some((range, offset - range.start)) } else { None }
}

/// 返回 `<script>` 块内容在源文件中的字节范围（不含标签本身）
pub fn script_offset_range(root: &AwslRoot, source: &str) -> Option<std::ops::Range<usize>> {
    let script = root.script.as_ref()?;
    let open = "<script";
    let open_pos = source.find(open)?;
    let body_start = source[open_pos..].find('>')? + open_pos + 1;
    let close = "</script>";
    let close_pos = source[body_start..].find(close)? + body_start;
    if close_pos > body_start { Some(body_start..close_pos) } else { None }
}
