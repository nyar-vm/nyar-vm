//! `<script>` 正式脚本事实：经共享 Oak parser 一次性解析，供终端生成器消费。
//!
//! 合同（账本条目 46）：产品生成器不得解析脚本字符串。函数声明、语句、赋值、
//! 引用与字面量事实只来自本模块持有的 Oak AST；语法非法的脚本在解析入口
//! 结构化失败，不进入生成。模板插值/循环源等表达式片段也不是完整程序，
//! 包装成正式 `let` 声明经同一 parser 解析，不做文本级回退。

use oak_core::Range;
use oak_valkyrie::ast::{MicroDeclaration, Pattern, StatementNode, TermExpression};

use crate::asgard::error::{AsgardError, AsgardResult};
use crate::compiler::syntax::parser::{ValkyrieRoot, parse_source};

/// 一个组件 `<script>` 的正式解析事实（AST + 原文，供引号捕获等 span 切片）。
pub struct ScriptFacts {
    source: String,
    root: ValkyrieRoot,
}

impl ScriptFacts {
    /// 用正式 parser 解析脚本；任何语法诊断都拒绝生成，不做错误恢复后再消费。
    pub fn parse(script: &str) -> AsgardResult<Self> {
        let root = parse_source(script).map_err(|error| {
            AsgardError::msg(report!("<script> 不是合法 V 源码，生成器拒绝消费：{error}"))
        })?;
        Ok(Self { source: script.to_string(), root })
    }

    /// 按名取 `micro` 声明（含 namespace 体）；同名声明缺席返回 `None`。
    pub fn handler(&self, name: &str) -> Option<&MicroDeclaration> {
        fn walk<'a>(items: &'a [StatementNode], name: &str) -> Option<&'a MicroDeclaration> {
            items.iter().find_map(|item| match item {
                StatementNode::Micro(micro) if micro.name.name == name => Some(micro.as_ref()),
                StatementNode::Namespace(namespace) => {
                    walk(namespace.body.as_ref().map(|body| body.items.as_slice()).unwrap_or(&[]), name)
                }
                _ => None,
            })
        }
        walk(&self.root.items, name)
    }

    /// 顶层简单绑定的初始表达式（同一 AST 事实；非响应式绑定的读取展开来源）。
    pub fn binding_initializer(&self, name: &str) -> Option<&TermExpression> {
        fn walk<'a>(items: &'a [StatementNode], name: &str) -> Option<&'a TermExpression> {
            items.iter().find_map(|item| match item {
                StatementNode::Let(declaration) => match &declaration.pattern {
                    Pattern::Variable(variable) if variable.name.name == name => declaration.expr.as_ref(),
                    _ => None,
                },
                StatementNode::Namespace(namespace) => {
                    walk(namespace.body.as_ref().map(|body| body.items.as_slice()).unwrap_or(&[]), name)
                }
                _ => None,
            })
        }
        walk(&self.root.items, name)
    }

    /// 原文切片：引号捕获等需要保留原始字节的事实从这里取。
    pub fn slice(&self, span: Range<usize>) -> &str {
        self.source.get(span.start..span.end).unwrap_or_default()
    }

    /// 脚本原文（表达式渲染器的引号捕获切片来源）。
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// 表达式片段的解析事实：AST + 承载它的包装原文（切片引用区间用）。
pub struct FragmentFacts {
    /// 包装程序中 `__awsl_probe` 的初始化表达式 AST。
    pub expr: TermExpression,
    /// 包装源码 `let __awsl_probe = <fragment>;`；片段内引号捕获的 span 指向它。
    pub source: String,
}

/// 把模板插值/循环源/条件等表达式片段用正式 parser 解析成表达式 AST。
///
/// 解析失败结构化报错并带原文片段，不做字符串级猜测。
pub fn parse_expression_fragment(fragment: &str) -> AsgardResult<FragmentFacts> {
    let trimmed = fragment.trim();
    if trimmed.is_empty() {
        return Err(AsgardError::msg("表达式片段为空，没有可解析的表达式事实"));
    }
    let source = format!("let __awsl_probe = {trimmed};");
    let root = parse_source(&source)
        .map_err(|error| AsgardError::msg(report!("表达式片段 `{trimmed}` 不是合法 V 表达式，拒绝消费：{error}")))?;
    for item in &root.items {
        if let StatementNode::Let(declaration) = item {
            if let Some(expr) = &declaration.expr {
                return Ok(FragmentFacts { expr: expr.clone(), source });
            }
        }
    }
    Err(AsgardError::msg(report!("表达式片段 `{trimmed}` 未产出初始化表达式")))
}
