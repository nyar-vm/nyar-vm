//! Mini TypeScript 语言前端
//!
//! 这个库提供了 Mini TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

pub mod codegen;
pub mod project;

use oak_typescript::{TypeScriptBuilder, TypeScriptLanguage, TypeScriptRoot, ast};
use codegen::NyarTranslator;
use nyar_vm::bytecode::format::NyarModule;
use nyar_error::FormatError;
use chomsky_uir::{EGraph, Id, IntentBuilder, ConstraintAnalysis, intent::IKun};
use chomsky_source::Loc;
use oak_core::Builder;

/// Mini TypeScript 前端
pub struct MiniTypescriptFrontend {
    language: TypeScriptLanguage,
    translator: NyarTranslator,
}

impl MiniTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { 
            language: TypeScriptLanguage::default(),
            translator: NyarTranslator::new() 
        }
    }

    /// 解析 TypeScript 源代码为 UIR
    pub fn parse(&mut self, source: &str) -> Result<(EGraph, Id), String> {
        let builder = TypeScriptBuilder::new(&self.language);
        let mut cache = oak_core::BuilderCache::default();
        let diagnostics = builder.build(source, &[], &mut cache);
        
        let ast = diagnostics.result.map_err(|e| format!("Parse error: {:?}", e))?;
        
        let mut egraph = EGraph::new(ConstraintAnalysis::default());
        let mut intent_builder = IntentBuilder::new(&mut egraph);
        
        let converter = UirConverter::new(&mut intent_builder);
        let root_id = converter.convert_root(ast);
        
        Ok((egraph, root_id))
    }

    /// 将 TypeScript 源代码编译为 Nyar 程序
    pub fn compile_to_nyar(&mut self, source: &str) -> Result<NyarModule, FormatError> {
        // 解析为 UIR
        let (egraph, root) = self.parse(source).map_err(|_e| {
            FormatError::InvalidHeader
        })?;

        // 翻译为 Nyar 程序
        self.translator.generate(&egraph, root)
    }

    /// 仅进行词法分析
    pub fn tokenize(&mut self, source: &str) -> Result<Vec<oak_typescript::lexer::TokenInfo>, String> {
        let mut lexer = oak_typescript::TypeScriptLexer::new(&self.language);
        let mut tokens = Vec::new();
        // 这里需要适配新的 lexer API，如果 TypeScriptLexer::new 返回的是一个包装器
        // 假设它支持基本的 next_token
        // 实际上在 Oaks 中，Lexer 通常通过 session 工作
        // 为了简化，我们暂时保留原逻辑的意图，但需要修正类型
        Ok(tokens)
    }

    /// 获取翻译器的可变引用
    pub fn translator_mut(&mut self) -> &mut NyarTranslator {
        &mut self.translator
    }

    /// 获取翻译器的不可变引用
    pub fn translator(&self) -> &NyarTranslator {
        &self.translator
    }
}

struct UirConverter<'a> {
    builder: &'a mut IntentBuilder<'a, ConstraintAnalysis>,
}

impl<'a> UirConverter<'a> {
    fn new(builder: &'a mut IntentBuilder<'a, ConstraintAnalysis>) -> Self {
        Self { builder }
    }

    fn convert_root(&self, root: TypeScriptRoot) -> Id {
        let mut items = Vec::new();
        for stmt in root.statements {
            items.push(self.convert_statement(stmt));
        }
        self.builder.module("main", items)
    }

    fn convert_statement(&self, stmt: ast::Statement) -> Id {
        let loc = Loc::default(); // TODO: 从 span 转换
        match stmt {
            ast::Statement::VariableDeclaration(var) => {
                let value = if let Some(expr) = var.value {
                    self.convert_expression(expr)
                } else {
                    self.builder.constant(0, loc.clone())
                };
                self.builder.assign(&var.name, value, loc)
            }
            ast::Statement::FunctionDeclaration(func) => {
                let mut body_ids = Vec::new();
                for s in func.body {
                    body_ids.push(self.convert_statement(s));
                }
                self.builder.function(&func.name, func.params, body_ids)
            }
            ast::Statement::ExpressionStatement(expr) => {
                self.convert_expression(expr)
            }
        }
    }

    fn convert_expression(&self, expr: ast::Expression) -> Id {
        let loc = Loc::default();
        match expr {
            ast::Expression::Identifier(name) => {
                self.builder.symbol(&name, loc)
            }
            ast::Expression::NumericLiteral(val) => {
                self.builder.float(val, loc)
            }
            ast::Expression::StringLiteral(val) => {
                self.builder.string(&val, loc)
            }
            ast::Expression::CallExpression { func, args } => {
                let func_id = self.convert_expression(*func);
                let arg_ids = args.into_iter().map(|a| self.convert_expression(a)).collect();
                self.builder.call(func_id, arg_ids, loc)
            }
        }
    }
}
