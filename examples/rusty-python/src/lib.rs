//! Mini Python 语言前端
//!
//! 这个库提供了 Mini Python 语言的词法分析、语法分析和 Gaia 翻译功能。

use chomsky_extract::IKunTree;
use nyar_types::{NyarError, NyarFrontend};
use oak_python::{ast::Program, PythonFrontend};
use oak_core::source::SourceText;

pub mod codegen;
pub mod pyc_codegen;

/// Mini Python 前端
#[derive(Default)]
pub struct MiniPythonFrontend;

impl MiniPythonFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self
    }
}

impl NyarFrontend for MiniPythonFrontend {
    type Language = oak_python::PythonLanguage;

    /// 解析 Python 源代码
    fn parse(&self, source: &str) -> Result<Program, NyarError> {
        let source_text = SourceText::new(source);
        let frontend = PythonFrontend::new(&source_text);
        frontend.parse_to_ast().map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    /// 编译到 Chomsky UIR (IKunTree)
    fn lower(&self, ast: &Program) -> Result<IKunTree, NyarError> {
        // Python 前端目前的 lower 逻辑散落在 PythonFrontend 中，
        // 这里需要适配。由于 PythonFrontend::parse 接受 IntentBuilder，
        // 我们需要创建一个 EGraph 和 Builder 来获取 IKunTree。
        use chomsky_uir::{EGraph, IntentBuilder, ConstraintAnalysis, IKun};
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        
        // 注意：PythonFrontend::parse 实际上处理的是解析+降级
        // 如果要完全对齐，PythonFrontend 应该提供一个 lower(ast) 的方法。
        // 暂时使用 SourceText 重新解析并降级，或者直接从 AST 降级。
        // 这里假设我们能从 AST 降级，或者直接使用现有解析流程。
        let source_text = SourceText::new(""); // 暂时占位，因为我们已经有 AST 了？
        // 实际上 PythonFrontend 的实现是直接从 SourceText 到 EGraph。
        // 为了对齐接口，我们在这里重新走一遍流程。
        // 以后建议重构 PythonFrontend 使其支持 AST -> EGraph。
        
        // 重新获取源码以进行解析+降级
        // 这是一个折中方案，因为 NyarFrontend 接口要求 parse 和 lower 分开。
        // 但 PythonFrontend 目前是合在一起的。
        Ok(IKunTree::Constant(0)) // 占位，待进一步重构 PythonFrontend
    }
}
