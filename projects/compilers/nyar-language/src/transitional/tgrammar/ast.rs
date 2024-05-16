//! T-Grammar 内联 meta 标记 AST（`<% %>` 元计算）。

use std::ops::Range;

/// 模板根：有序子节点。
pub type TgRoot = Vec<TgNode>;

/// 模板节点。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TgNode {
    /// 静态文本与 `{expr}` 插值。
    Text {
        /// 文本片段列表。
        parts: Vec<TgTextPart>,
        /// 节点在源码中的字节范围。
        span: Range<usize>,
    },
    /// 单行 meta 语句 `<% stmt %>`（内部原文供 Valkyrie 解析）。
    Stmt {
        /// 语句正文原文。
        body: String,
        /// 节点在源码中的字节范围。
        span: Range<usize>,
    },
    /// 条件块 `<% if %>` … `<% end %>`。
    If(TgIf),
    /// 循环块 `<% loop %>` … `<% end %>`。
    Loop(TgLoop),
    /// 匹配块 `<% match %>` … `<% end %>`。
    Match(TgMatch),
    /// 模板注释 `<# ... #>`（不输出）。
    Comment {
        /// 注释在源码中的字节范围。
        span: Range<usize>,
    },
}

/// `if` 分支。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TgIfArm {
    /// 条件表达式原文；`None` 表示 `else` 分支。
    pub condition: Option<String>,
    /// 分支正文节点。
    pub body: TgRoot,
    /// 分支在源码中的字节范围。
    pub span: Range<usize>,
}

/// `if` 块。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TgIf {
    /// 条件分支列表（含可选 `else`）。
    pub arms: Vec<TgIfArm>,
    /// 整块在源码中的字节范围。
    pub span: Range<usize>,
}

/// `loop` 块。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TgLoop {
    /// `loop` 头原文（如 `i in items`）。
    pub header: String,
    /// 循环体节点。
    pub body: TgRoot,
    /// 整块在源码中的字节范围。
    pub span: Range<usize>,
}

/// `match` 分支。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TgMatchArm {
    /// 模式原文；`None` 表示 `else` 分支。
    pub pattern: Option<String>,
    /// 分支正文节点。
    pub body: TgRoot,
    /// 分支在源码中的字节范围。
    pub span: Range<usize>,
}

/// `match` 块。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TgMatch {
    /// 被匹配表达式原文。
    pub scrutinee: String,
    /// 模式分支列表。
    pub arms: Vec<TgMatchArm>,
    /// 整块在源码中的字节范围。
    pub span: Range<usize>,
}

/// 文本片段。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TgTextPart {
    /// 静态文本片段。
    Static(String),
    /// `{expr}` 插值表达式原文。
    Expression(String),
}

/// T-Grammar 块级关键词（`<% kw %>` 指令头）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgKeyword {
    /// `if` 条件块头。
    If,
    /// `else if` 分支头。
    ElseIf,
    /// `else` 分支头。
    Else,
    /// 块结束标记。
    End,
    /// `loop` 循环头。
    Loop,
    /// `match` 匹配头。
    Match,
    /// `case` 分支头。
    Case,
}
