//! T-Grammar 词法记号。

use std::ops::Range;

/// T-Grammar 词法记号种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// 静态文本（可含 `{expr}`，由 parser 二次切分）。
    Text,
    /// `<% ... %>` 指令块。
    Directive,
    /// `<# ... #>` 注释。
    Comment,
    /// 输入结束。
    Eof,
}

/// 单个词法记号及其源码范围。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// 记号种类。
    pub kind: TokenKind,
    /// 记号在源码中的字节范围。
    pub span: Range<usize>,
}

impl Token {
    pub(crate) fn eof(offset: usize) -> Self {
        Self { kind: TokenKind::Eof, span: offset..offset }
    }
}
