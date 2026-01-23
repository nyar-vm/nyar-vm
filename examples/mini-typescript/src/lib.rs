//! Mini TypeScript 语言前端
//!
//! 这个库提供了 Mini TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

#![feature(new_range_api)]

pub mod codegen;
pub mod project;

use oak_core::{Builder, Lexer, SourceText, ParseSession};
use oak_typescript::{TypeScriptBuilder, TypeScriptLanguage, TypeScriptRoot, ast, TypeScriptSyntaxKind};
use codegen::NyarTranslator;
use nyar_vm::bytecode::format::NyarModule;
use nyar_error::FormatError;
use chomsky_uir::{EGraph, Id, IntentBuilder, ConstraintAnalysis, intent::IKun};
use chomsky_source::Loc;
use core::range::Range;

/// Mini TypeScript 前端
pub struct MiniTypescriptFrontend {
    language: TypeScriptLanguage,
    translator: NyarTranslator,
    source_id: u32,
}

impl MiniTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { 
            language: TypeScriptLanguage::standard(),
            translator: NyarTranslator::new(),
            source_id: 1, // 默认 source_id
        }
    }

    /// 设置当前处理的源码 ID
    pub fn set_source_id(&mut self, id: u32) {
        self.source_id = id;
    }

    /// 解析 TypeScript 源代码为 UIR
    pub fn parse(&mut self, source: &str) -> Result<(EGraph<IKun, ConstraintAnalysis>, Id), String> {
        let builder = TypeScriptBuilder::new(&self.language);
        let mut session = ParseSession::<TypeScriptLanguage>::default();
        let source_text = SourceText::new(source);
        let diagnostics = Builder::build(&builder, &source_text, &[], &mut session);
        
        let ast = diagnostics.result.map_err(|e| format!("Parse error: {:?}", e))?;
        
        let mut egraph = EGraph::new();
        let mut intent_builder = IntentBuilder::new(&mut egraph);
        
        let mut converter = UirConverter::new(&mut intent_builder, self.source_id);
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
    pub fn tokenize(&mut self, source: &str) -> Result<Vec<oak_core::lexer::Token<TypeScriptSyntaxKind>>, String> {
        let lexer = oak_typescript::TypeScriptLexer::new(&self.language);
        let mut session = ParseSession::<TypeScriptLanguage>::default();
        let source_text = SourceText::new(source);
        let output = Lexer::lex(&lexer, &source_text, &[], &mut session);
        
        if !output.diagnostics.is_empty() {
            return Err(format!("Lexer errors: {:?}", output.diagnostics));
        }
        
        let tokens = output.result.map_err(|e| format!("Lexer error: {:?}", e))?;
        Ok(tokens.to_vec())
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
    source_id: u32,
}

impl<'a> UirConverter<'a> {
    fn new(builder: &'a mut IntentBuilder<'a, ConstraintAnalysis>, source_id: u32) -> Self {
        Self { builder, source_id }
    }

    fn to_loc(&self, range: Range<usize>) -> Loc {
        Loc::new(self.source_id, range.start as u32, range.end as u32)
    }

    fn convert_root(&mut self, root: TypeScriptRoot) -> Id {
        let mut items = Vec::new();
        for stmt in root.statements {
            items.push(self.convert_statement(stmt));
        }
        self.builder.module("main", items)
    }

    fn convert_statement(&mut self, stmt: ast::Statement) -> Id {
        match stmt {
            ast::Statement::VariableDeclaration(var) => {
                let loc = self.to_loc(var.span);
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
            ast::Statement::ImportDeclaration(import) => {
                let loc = self.to_loc(import.span);
                let mut args = vec![self.builder.string(&import.module_specifier, loc.clone())];
                for s in import.imports {
                    args.push(self.builder.symbol(&s, loc.clone()));
                }
                self.builder.extension("import", args, loc)
            }
            ast::Statement::ExportDeclaration(export) => {
                let loc = self.to_loc(export.span);
                let inner = self.convert_statement(*export.declaration);
                self.builder.extension("export", vec![inner], loc)
            }
        }
    }

    fn convert_expression(&mut self, expr: ast::Expression) -> Id {
        match expr {
            ast::Expression::Identifier(name) => {
                self.builder.symbol(&name, Loc::default())
            }
            ast::Expression::NumericLiteral(val) => {
                self.builder.constant(val as i64, Loc::default())
            }
            ast::Expression::StringLiteral(val) => {
                self.builder.string(&val, Loc::default())
            }
            ast::Expression::BooleanLiteral(val) => {
                self.builder.bool(val, Loc::default())
            }
            ast::Expression::BinaryExpression { left, operator, right } => {
                let l = self.convert_expression(*left);
                let r = self.convert_expression(*right);
                self.builder.binary_op(&operator, l, r, Loc::default())
            }
            ast::Expression::CallExpression { func, args } => {
                let f = self.convert_expression(*func);
                let mut arg_ids = Vec::new();
                for arg in args {
                    arg_ids.push(self.convert_expression(arg));
                }
                self.builder.call(f, arg_ids, Loc::default())
            }
            ast::Expression::UnaryExpression { operator, argument } => {
                let arg = self.convert_expression(*argument);
                self.builder.extension(&operator, vec![arg], Loc::default())
            }
            ast::Expression::MemberExpression { object, property, computed, .. } => {
                let obj = self.convert_expression(*object);
                let prop = self.convert_expression(*property);
                let op = if computed { "index" } else { "member" };
                self.builder.extension(op, vec![obj, prop], Loc::default())
            }
            ast::Expression::ConditionalExpression { test, consequent, alternate } => {
                let t = self.convert_expression(*test);
                let c = self.convert_expression(*consequent);
                let a = self.convert_expression(*alternate);
                self.builder.branch(t, c, a, Loc::default())
            }
            _ => {
                self.builder.constant(0, Loc::default())
            }
        }
    }
}
