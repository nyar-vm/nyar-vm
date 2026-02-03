//! Rusty Swift 语言前端
//!
//! 这个库提供了 Rusty Swift 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarContext, NyarError, NyarFrontend};
use oak_vfs::Vfs;
use oak_core::source::SourceText;
use oak_swift::ast::{Expression, Literal, Statement, SwiftRoot};
use oak_swift::{SwiftBuilder, SwiftLanguage};
use chomsky_uir::Id;
use chomsky_types::Loc;

/// Rusty Swift 前端
pub struct RustySwiftFrontend {
    language: SwiftLanguage,
}

impl Default for RustySwiftFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustySwiftFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: SwiftLanguage::default(),
        }
    }
}

impl NyarFrontend for RustySwiftFrontend {
    type Language = SwiftLanguage;

    fn parse(&self, source: &str) -> Result<SwiftRoot, NyarError> {
        let builder = SwiftBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<SwiftLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &SwiftRoot, ctx: &mut NyarContext<V>) -> Id {
        let mut converter = UirConverter::new(ctx);
        converter.convert_root(ast)
    }
}

struct UirConverter<'a, 'b, V: Vfs, A: chomsky_uir::Analysis<chomsky_uir::IKun>> {
    ctx: &'a mut NyarContext<'b, V, A>,
}

impl<'a, 'b, V: Vfs, A: chomsky_uir::Analysis<chomsky_uir::IKun>> UirConverter<'a, 'b, V, A> {
    fn new(ctx: &'a mut NyarContext<'b, V, A>) -> Self {
        Self { ctx }
    }

    fn convert_root(&mut self, root: &SwiftRoot) -> Id {
        let mut items = Vec::new();
        for stmt in &root.program.statements {
            if let Some(node) = self.convert_statement(stmt) {
                items.push(node);
            }
        }
        self.ctx.builder().module("main", items)
    }

    fn convert_statement(&mut self, stmt: &Statement) -> Option<Id> {
        let loc = Loc::default();
        match stmt {
            Statement::VariableDecl { name, value, .. } => {
                let val = if let Some(v) = value {
                    self.convert_expression(v)
                } else {
                    self.ctx.builder().extension("nil", vec![], loc.clone())
                };
                let name_id = self.ctx.scopes.declare_variable(name);
                Some(self.ctx.builder().assign(&name_id, val, loc))
            }
            Statement::Expression(expr) => Some(self.convert_expression(expr)),
            Statement::Return(expr) => {
                let val = if let Some(v) = expr {
                    self.convert_expression(v)
                } else {
                    self.ctx.builder().extension("nil", vec![], loc.clone())
                };
                Some(self.ctx.builder().return_(val, loc))
            }
            Statement::If { test, body, orelse } => {
                let cond = self.convert_expression(test);
                let then_body = self.convert_statements(body);
                let else_body = orelse.as_ref().map(|b| self.convert_statements(b)).unwrap_or_else(|| self.ctx.builder().block(vec![], loc.clone()));
                Some(self.ctx.builder().branch(cond, then_body, else_body, loc))
            }
            Statement::While { test, body } => {
                let cond = self.convert_expression(test);
                let loop_body = self.convert_statements(body);
                Some(self.ctx.builder().while_loop(cond, loop_body, loc))
            }
            Statement::FunctionDef { name, body, .. } => {
                let mut func_body = Vec::new();
                for stmt in body {
                    if let Some(id) = self.convert_statement(stmt) {
                        func_body.push(id);
                    }
                }
                Some(self.ctx.builder().function(name, vec![], func_body))
            }
            Statement::Block(stmts) => Some(self.convert_statements(stmts)),
        }
    }

    fn convert_statements(&mut self, stmts: &[Statement]) -> Id {
        let loc = Loc::default();
        let mut items = Vec::new();
        for stmt in stmts {
            if let Some(node) = self.convert_statement(stmt) {
                items.push(node);
            }
        }
        self.ctx.builder().block(items, loc)
    }

    fn convert_expression(&mut self, expr: &Expression) -> Id {
        let loc = Loc::default();
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Number(n) => self.ctx.builder().constant(n.parse().unwrap_or(0), loc),
                Literal::String(s) => self.ctx.builder().string(s, loc),
                Literal::Boolean(b) => self.ctx.builder().bool(*b, loc),
                Literal::Nil => self.ctx.builder().extension("nil", vec![], loc),
            },
            Expression::Identifier(name) => {
                let var = self.ctx.scopes.resolve_variable(name);
                self.ctx.builder().symbol(&var, loc)
            }
            Expression::Binary { left, operator, right } => {
                let l = self.convert_expression(left);
                let r = self.convert_expression(right);
                let op = match operator.as_str() {
                    "+" => "add",
                    "-" => "sub",
                    "*" => "mul",
                    "/" => "div",
                    "==" => "eq",
                    "!=" => "ne",
                    "<" => "lt",
                    ">" => "gt",
                    "<=" => "le",
                    ">=" => "ge",
                    _ => "unknown",
                };
                self.ctx.builder().extension(op, vec![l, r], loc)
            }
            Expression::Call { callee, arguments } => {
                let func = self.convert_expression(callee);
                let args = arguments.iter().map(|a| self.convert_expression(a)).collect();
                self.ctx.builder().call(func, args, loc)
            }
            Expression::Unary { operator, operand } => {
                let val = self.convert_expression(operand);
                let op = match operator.as_str() {
                    "-" => "neg",
                    "!" => "not",
                    _ => "unknown",
                };
                self.ctx.builder().extension(op, vec![val], loc)
            }
            Expression::Member { object, member } => {
                let obj = self.convert_expression(object);
                let attr = self.ctx.builder().symbol(member, loc.clone());
                self.ctx.builder().extension("get_field", vec![obj, attr], loc)
            }
        }
    }
}
