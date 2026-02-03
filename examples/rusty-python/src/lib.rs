//! Rusty Python 语言前端
//!
//! 这个库提供了 Rusty Python 语言的词法分析、语法分析和 Gaia 翻译功能。

use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_python::ast::{Expression, Literal, PythonRoot, Statement};
use chomsky_uir::Id;
use chomsky_source::Loc;

pub mod codegen;
pub mod pyc_codegen;

/// Rusty Python 前端
#[derive(Default)]
pub struct RustyPythonFrontend;

impl RustyPythonFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self
    }
}

impl NyarFrontend for RustyPythonFrontend {
    type Language = oak_python::PythonLanguage;

    fn parse(&self, source: &str) -> Result<PythonRoot, NyarError> {
        let config = oak_python::PythonLanguage {};
        let parser = oak_python::PythonParser::new(&config);
        let mut cache =
            oak_core::parser::session::ParseSession::<oak_python::PythonLanguage>::default();
        let parse_result = parser.parse(source, &[], &mut cache);

        let green_node = parse_result
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))?;

        let builder = oak_python::PythonBuilder::new(&config);
        let source_text = oak_core::source::SourceText::new(source.to_string());
        let ast = builder
            .build_root(green_node, &source_text)
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))?;
        Ok(ast)
    }

    fn lower_unified(&self, ast: &PythonRoot, ctx: &mut NyarContext) -> Id {
        let mut converter = UirConverter::new(ctx);
        converter.convert_root(ast)
    }
}

struct UirConverter<'a, 'b, A: chomsky_uir::Analysis<chomsky_uir::IKun>> {
    ctx: &'a mut NyarContext<'b, A>,
}

impl<'a, 'b, A: chomsky_uir::Analysis<chomsky_uir::IKun>> UirConverter<'a, 'b, A> {
    fn new(ctx: &'a mut NyarContext<'b, A>) -> Self {
        Self { ctx }
    }

    fn convert_root(&mut self, root: &PythonRoot) -> Id {
        let mut items = Vec::new();
        for stmt in &root.program.statements {
            if let Some(node) = self.convert_statement(stmt) {
                items.push(node);
            }
        }
        self.ctx.builder().module("main", items)
    }

    fn convert_statement(&mut self, stmt: &Statement) -> Option<Id> {
        let loc = Loc::default(); // Python AST lacks spans
        match stmt {
            Statement::Assignment { target, value } => {
                let val = self.convert_expression(value);
                if let Expression::Name(name) = target {
                    let name = self.ctx.scopes.declare_variable(name);
                    Some(self.ctx.builder().assign(&name, val, loc))
                } else {
                    let target_id = self.convert_expression(target);
                    Some(self.ctx.builder().assign_to_id(target_id, val, loc))
                }
            }
            Statement::AugmentedAssignment {
                target,
                operator,
                value,
            } => {
                let target_node = self.convert_expression(target);
                let value_node = self.convert_expression(value);
                let op_name = match operator {
                    oak_python::ast::AugmentedOperator::Add => "add",
                    oak_python::ast::AugmentedOperator::Sub => "sub",
                    oak_python::ast::AugmentedOperator::Mult => "mul",
                    oak_python::ast::AugmentedOperator::Div => "div",
                    oak_python::ast::AugmentedOperator::FloorDiv => "floordiv",
                    oak_python::ast::AugmentedOperator::Mod => "mod",
                    oak_python::ast::AugmentedOperator::Pow => "pow",
                    oak_python::ast::AugmentedOperator::LShift => "lshift",
                    oak_python::ast::AugmentedOperator::RShift => "rshift",
                    oak_python::ast::AugmentedOperator::BitOr => "bitor",
                    oak_python::ast::AugmentedOperator::BitXor => "bitxor",
                    oak_python::ast::AugmentedOperator::BitAnd => "bitand",
                };
                let result_node = self.ctx.builder().binary_op(op_name, target_node, value_node, loc.clone());
                Some(self.ctx.builder().assign_to_id(target_node, result_node, loc))
            }
            Statement::Expression(expr) => Some(self.convert_expression(expr)),
            Statement::FunctionDef {
                name,
                parameters,
                body,
                ..
            } => {
                self.ctx.scopes.push_scope();
                let params = parameters.iter().map(|p| self.ctx.scopes.declare_variable(&p.name)).collect();
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.convert_statement(s) {
                        body_items.push(node);
                    }
                }
                let body_id = self.ctx.builder().block(body_items, loc.clone());
                self.ctx.scopes.pop_scope();
                
                let lambda = self.ctx.builder().function(name, params, vec![body_id]);
                Some(self.ctx.builder().assign(name, lambda, loc))
            }
            Statement::Return(expr) => {
                let val = expr
                    .as_ref()
                    .map(|e| self.convert_expression(e))
                    .unwrap_or(self.ctx.builder().constant(0, loc.clone()));
                Some(self.ctx.builder().return_(val, loc))
            }
            Statement::If { test, body, orelse } => {
                let cond = self.convert_expression(test);
                let mut then_items = Vec::new();
                for s in body {
                    if let Some(node) = self.convert_statement(s) {
                        then_items.push(node);
                    }
                }
                let then_id = self.ctx.builder().block(then_items, loc.clone());
                
                let mut else_items = Vec::new();
                for s in orelse {
                    if let Some(node) = self.convert_statement(s) {
                        else_items.push(node);
                    }
                }
                let else_id = self.ctx.builder().block(else_items, loc.clone());
                
                Some(self.ctx.builder().branch(cond, then_id, else_id, loc))
            }
            Statement::While { test, body, .. } => {
                let cond = self.convert_expression(test);
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.convert_statement(s) {
                        body_items.push(node);
                    }
                }
                let body_id = self.ctx.builder().block(body_items, loc.clone());
                Some(self.ctx.builder().while_loop(cond, body_id, loc))
            }
            Statement::For {
                target,
                iter,
                body,
                ..
            } => {
                let target_node = self.convert_expression(target);
                let iter_node = self.convert_expression(iter);
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.convert_statement(s) {
                        body_items.push(node);
                    }
                }
                let body_id = self.ctx.builder().block(body_items, loc.clone());
                Some(self.ctx.builder().extension("foreach", vec![target_node, iter_node, body_id], loc))
            }
            Statement::Pass => Some(self.ctx.builder().constant(0, loc)),
            Statement::Break => Some(self.ctx.builder().extension("break", vec![], loc)),
            Statement::Continue => Some(self.ctx.builder().extension("continue", vec![], loc)),
            _ => None,
        }
    }

    fn convert_expression(&mut self, expr: &Expression) -> Id {
        let loc = Loc::default();
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Integer(i) => self.ctx.builder().constant(*i, loc),
                Literal::Float(f) => self.ctx.builder().constant(f.to_bits() as i64, loc), // FIXME: use float
                Literal::String(_s) => self.ctx.builder().extension("string", vec![], loc),
                Literal::Boolean(b) => self.ctx.builder().constant(if *b { 1 } else { 0 }, loc),
                Literal::None => self.ctx.builder().constant(0, loc),
            },
            Expression::Name(name) => {
                let resolved = self.ctx.scopes.resolve_variable(name);
                self.ctx.builder().symbol(&resolved, loc)
            }
            Expression::BinaryOp {
                left,
                operator,
                right,
            } => {
                let left_node = self.convert_expression(left);
                let right_node = self.convert_expression(right);
                let op_name = match operator {
                    oak_python::ast::BinaryOperator::Add => "add",
                    oak_python::ast::BinaryOperator::Sub => "sub",
                    oak_python::ast::BinaryOperator::Mult => "mul",
                    oak_python::ast::BinaryOperator::Div => "div",
                    oak_python::ast::BinaryOperator::FloorDiv => "floordiv",
                    oak_python::ast::BinaryOperator::Mod => "mod",
                    oak_python::ast::BinaryOperator::Pow => "pow",
                    oak_python::ast::BinaryOperator::LShift => "lshift",
                    oak_python::ast::BinaryOperator::RShift => "rshift",
                    oak_python::ast::BinaryOperator::BitOr => "bitor",
                    oak_python::ast::BinaryOperator::BitXor => "bitxor",
                    oak_python::ast::BinaryOperator::BitAnd => "bitand",
                };
                self.ctx.builder().binary_op(op_name, left_node, right_node, loc)
            }
            Expression::UnaryOp { operator, operand } => {
                let operand_node = self.convert_expression(operand);
                let op_name = match operator {
                    oak_python::ast::UnaryOperator::Invert => "invert",
                    oak_python::ast::UnaryOperator::Not => "not",
                    oak_python::ast::UnaryOperator::UAdd => "uadd",
                    oak_python::ast::UnaryOperator::USub => "usub",
                };
                self.ctx.builder().extension(op_name, vec![operand_node], loc)
            }
            Expression::BoolOp { operator, values } => {
                let op_name = match operator {
                    oak_python::ast::BoolOperator::And => "and",
                    oak_python::ast::BoolOperator::Or => "or",
                };
                let nodes = values.iter().map(|v| self.convert_expression(v)).collect();
                self.ctx.builder().extension(op_name, nodes, loc)
            }
            Expression::List { elts } => {
                let nodes = elts.iter().map(|e| self.convert_expression(e)).collect();
                self.ctx.builder().extension("list", nodes, loc)
            }
            Expression::Tuple { elts } => {
                let nodes = elts.iter().map(|e| self.convert_expression(e)).collect();
                self.ctx.builder().extension("tuple", nodes, loc)
            }
            Expression::Compare {
                left,
                ops,
                comparators,
            } => {
                let left_node = self.convert_expression(left);
                let right_node = self.convert_expression(&comparators[0]);
                let op_name = match ops[0] {
                    oak_python::ast::CompareOperator::Eq => "eq",
                    oak_python::ast::CompareOperator::NotEq => "noteq",
                    oak_python::ast::CompareOperator::Lt => "lt",
                    oak_python::ast::CompareOperator::LtE => "lte",
                    oak_python::ast::CompareOperator::Gt => "gt",
                    oak_python::ast::CompareOperator::GtE => "gte",
                    _ => "unknown",
                };
                self.ctx.builder().binary_op(op_name, left_node, right_node, loc)
            }
            Expression::Call { func, args, .. } => {
                let mut arguments = Vec::new();
                for arg in args {
                    arguments.push(self.convert_expression(arg));
                }

                if let Expression::Name(name) = &**func {
                    if let Some(intrinsic) = self.ctx.map_intrinsic(name, arguments.clone(), loc.clone()) {
                        return intrinsic;
                    }
                }

                let f = self.convert_expression(func);
                self.ctx.builder().call(f, arguments, loc)
            }
            _ => self.ctx.builder().constant(0, loc),
        }
    }
}
