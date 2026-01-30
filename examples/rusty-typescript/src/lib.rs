//! Mini TypeScript 语言前端
//!
//! 这个库提供了 Mini TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

#![feature(new_range_api)]

pub mod codegen;
pub mod project;
pub mod type_system;
pub mod errors;

use chomsky_extract::IKunExtractor;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IntentBuilder, Loc, Id};
use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{ParseSession, SourceText};
use oak_typescript::{ast, TypeScriptBuilder, TypeScriptLanguage, TypeScriptRoot};
use std::ops::Range;

/// Mini TypeScript 前端
pub struct MiniTypescriptFrontend {
    language: TypeScriptLanguage,
    source_id: u32,
}

impl Default for MiniTypescriptFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: TypeScriptLanguage::standard(),
            source_id: 1, // 默认 source_id
        }
    }

    /// 设置当前处理的源码 ID
    pub fn set_source_id(&mut self, id: u32) {
        self.source_id = id;
    }
}

impl NyarFrontend for MiniTypescriptFrontend {
    type Language = TypeScriptLanguage;

    fn parse(&self, source: &str) -> Result<TypeScriptRoot, NyarError> {
        let builder = TypeScriptBuilder::new(&self.language);
        let mut session = ParseSession::<TypeScriptLanguage>::default();
        let source_text = SourceText::new(source);
        let output = oak_core::Builder::build(&builder, &source_text, &[], &mut session);

        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, ast: &TypeScriptRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        let mut converter = UirConverter::new(&mut builder, self.source_id);

        let root_id = converter.convert_root(ast.clone());
        let extractor = IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL);
        Ok(extractor.extract(root_id))
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
                let loc = self.to_loc(func.span.clone());
                let mut body_ids = Vec::new();
                for s in func.body {
                    body_ids.push(self.convert_statement(s));
                }
                let lambda = self.builder.function(&func.name, func.params, body_ids);
                self.builder.assign(&func.name, lambda, loc)
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
