#![feature(new_range_api)]
//! Mini C 语言前端
//!
//! 这个库提供了 Mini C 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod ast;
pub mod codegen;
pub mod config;
pub mod converter;

pub use chomsky_full::optimizer::UniversalOptimizer;
use chomsky_uir::{IntentBuilder, egraph::EGraph};
pub use oak_c::{CLanguage, CLexer, CParser, CRoot};
use codegen::GaiaTranslator;
use config::ReadConfig;
use gaia_assembler::assembler::GaiaAssembler;
use gaia_assembler::program::GaiaModule;
use gaia_types::{GaiaError, helpers::CompilationTarget};
use oak_core::{
    parser::{ParseSession, Parser},
    source::SourceText,
};

/// Mini C 前端
pub struct MiniCFrontend {
    _config: ReadConfig,
    translator: GaiaTranslator,
    language: CLanguage,
}

impl MiniCFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            _config: ReadConfig::new(),
            translator: GaiaTranslator::new(),
            language: CLanguage::default(),
        }
    }

    /// 解析 C 源代码为 GreenNode
    pub fn parse<'a>(
        &self,
        source: &'a SourceText,
        session: &'a mut ParseSession<CLanguage>,
    ) -> Result<oak_core::tree::RedNode<'a, CLanguage>, GaiaError> {
        let parser = CParser::new(&self.language);

        let output = parser.parse(source, &[], session);

        if let Ok(root) = output.result {
            Ok(oak_core::tree::RedNode::new(root, 0))
        } else {
            Err(GaiaError::syntax_error(
                "Parse failed",
                gaia_types::SourceLocation::default(),
            ))
        }
    }

    /// 将 C 源代码编译为 Gaia 程序
    pub fn compile_to_gaia(&mut self, source: &str) -> Result<GaiaModule, GaiaError> {
        // 创建解析会话
        let mut session = ParseSession::new(16);

        // 解析为 RedNode
        let source_text = SourceText::new(source);
        let red_node = self.parse(&source_text, &mut session)?;

        // 转换 RedNode 到 UIR (Intent Builder)
        let mut egraph = EGraph::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        let root = converter::red_to_uir(&red_node, source, &mut builder);

        // 应用 ProjectChomsky 优化
        // let optimized_root = self.optimize_uir(&mut egraph, root);

        // 转换 UIR 回 MiniC AST
        let minic_ast = converter::uir_to_minic(&egraph, root);

        self.translator.generate(&minic_ast)
    }

    /// 将 C 源代码直接编译为二进制
    pub fn compile_to_binary(
        &mut self,
        source: &str,
        target: CompilationTarget,
    ) -> Result<Vec<u8>, GaiaError> {
        let module = self.compile_to_gaia(source)?;
        let assembler = GaiaAssembler::new();
        let generated_files = assembler.compile(&module, &target)?;

        // 返回主要的二进制文件
        if let Some((_, bytes)) = generated_files.files.iter().next() {
            Ok(bytes.clone())
        } else {
            Err(GaiaError::invalid_data("No output files generated"))
        }
    }

    /// 获取翻译器的可变引用
    pub fn translator_mut(&mut self) -> &mut GaiaTranslator {
        &mut self.translator
    }

    /// 获取翻译器的不可变引用
    pub fn translator(&self) -> &GaiaTranslator {
        &self.translator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mini_c_frontend() {
        let source = r#"
int add(int a, int b) {
    return a + b;
}

char* message = "Hello, World!";
int count = 42;
"#;

        let frontend = MiniCFrontend::new();
        let mut session = ParseSession::new(16);
        let root = frontend.parse(source, &mut session).unwrap();

        // 验证解析成功
        assert!(root.green.children().count() > 0);
    }

    #[test]
    fn test_compile_to_gaia() {
        let code = r#"
int main() {
    return 0;
}
"#;
        let mut frontend = MiniCFrontend::new();
        let result = frontend.compile_to_gaia(code);
        assert!(result.is_ok());
    }
}
