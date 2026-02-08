//! Rusty Julia 语言前端
//!
//! 这个库提供了 Rusty Julia 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_julia::{JuliaBuilder, JuliaLanguage, JuliaRoot};
use oak_julia::ast::{JuliaExpression, JuliaStatement};
use chomsky_uir::Id;
use chomsky_types::Loc;

/// Rusty Julia 前端
pub struct RustyJuliaFrontend {
    language: JuliaLanguage,
}

impl Default for RustyJuliaFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyJuliaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: JuliaLanguage::default(),
        }
    }
}

impl NyarFrontend for RustyJuliaFrontend {
    type Language = JuliaLanguage;

    fn parse(&self, source: &str) -> Result<JuliaRoot, NyarError> {
        let builder = JuliaBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<JuliaLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: oak_vfs::Vfs>(&self, ast: &JuliaRoot, ctx: &mut NyarContext<V>) -> Id {
        let mut converter = UirConverter::new(ctx);
        converter.convert_root(ast)
    }
}

struct UirConverter<'a, 'b, V: oak_vfs::Vfs> {
    ctx: &'a mut NyarContext<'b, V>,
}

impl<'a, 'b, V: oak_vfs::Vfs> UirConverter<'a, 'b, V> {
    fn new(ctx: &'a mut NyarContext<'b, V>) -> Self {
        Self { ctx }
    }

    fn convert_root(&mut self, root: &JuliaRoot) -> Id {
        let mut items = Vec::new();
        for stmt in &root.statements {
            if let Some(node) = self.convert_statement(stmt) {
                items.push(node);
            }
        }
        self.ctx.builder().module("main", items)
    }

    fn convert_statement(&mut self, stmt: &JuliaStatement) -> Option<Id> {
        match stmt {
            JuliaStatement::Function(func) => {
                self.ctx.scopes.push_scope();
                let mut body = Vec::new();
                for s in &func.body {
                    if let Some(node) = self.convert_statement(s) {
                        body.push(node);
                    }
                }
                self.ctx.scopes.pop_scope();
                let func_id = self.ctx.builder().function(&func.name, Vec::new(), body);
                Some(func_id)
            }
            JuliaStatement::Expression(expr) => Some(self.convert_expression(expr)),
            JuliaStatement::Error => None,
        }
    }

    fn convert_expression(&mut self, expr: &JuliaExpression) -> Id {
        let loc = Loc::default();
        match expr {
            JuliaExpression::Identifier(name) => {
                self.ctx.builder().symbol(name, loc)
            }
            JuliaExpression::Literal(val) => {
                let unquoted = if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\'')) {
                    &val[1..val.len() - 1]
                } else {
                    val
                };
                self.ctx.builder().string(unquoted, loc)
            }
            JuliaExpression::Binary { left, op, right } => {
                let lhs = self.convert_expression(left);
                let rhs = self.convert_expression(right);
                self.ctx.builder().extension(op, vec![lhs, rhs], loc)
            }
            JuliaExpression::Call { callee, arguments } => {
                let args = arguments.iter().map(|arg| self.convert_expression(arg)).collect();
                if let JuliaExpression::Identifier(name) = &**callee {
                    match name.as_str() {
                        "println" => return self.ctx.builder().cross_lang_call("nyar", "io", "println", args, loc),
                        "print" => return self.ctx.builder().cross_lang_call("nyar", "io", "print", args, loc),
                        _ => {}
                    }
                }
                let callee_id = self.convert_expression(callee);
                self.ctx.builder().call(callee_id, args, loc)
            }
        }
    }
}
