//! Mini Python 语言前端
//!
//! 这个库提供了 Mini Python 语言的词法分析、语法分析和 Gaia 翻译功能。

use nyar_types::{NyarError, NyarFrontend, IKunTree};
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
    fn lower(&self, _ast: &Program) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 Program 到 IKunTree 的转换
        let mut tree = IKunTree::default();
        tree.name = "mini-python-program".to_string();
        Ok(tree)
    }
}
