//! Mini Python 语言前端
//!
//! 这个库提供了 Mini Python 语言的词法分析、语法分析和 Gaia 翻译功能。

use nyar_types::{NyarError, NyarFrontend, IKunTree};
use oak_python::ast::{Program, PythonRoot};

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

    fn parse(&self, source: &str) -> Result<PythonRoot, NyarError> {
        let config = oak_python::PythonLanguage;
        let parser = oak_python::PythonParser::new(config);
        let ast = parser.parse(source).map_err(|e| NyarError::Parse(e.to_string()))?;
        Ok(ast)
    }

    fn lower(&self, _ast: &PythonRoot) -> Result<IKunTree, NyarError> {
        // TODO: implement lowering from Python AST to IKunTree
        let tree = IKunTree::Module("mini-python-program".to_string(), vec![]);
        Ok(tree)
    }
}
