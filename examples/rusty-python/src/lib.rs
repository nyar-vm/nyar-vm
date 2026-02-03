//! Rusty Python 语言前端
//!
//! 这个库提供了 Rusty Python 语言的词法分析、语法分析和 Gaia 翻译功能。

use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_python::ast::{Expression, Literal, PythonRoot, Statement};
use chomsky_uir::Id;
use chomsky_types::Loc;

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
                match target {
                    Expression::Name(name) => {
                        let name = self.ctx.scopes.declare_variable(name);
                        Some(self.ctx.builder().assign(&name, val, loc))
                    }
                    Expression::Attribute { value: obj, attr } => {
                        let obj_node = self.convert_expression(obj);
                        let attr_id = self.ctx.builder().symbol(attr, loc.clone());
                        Some(self.ctx.builder().extension("set_field", vec![obj_node, attr_id, val], loc))
                    }
                    _ => {
                        let target_id = self.convert_expression(target);
                        Some(self.ctx.builder().assign_to_id(target_id, val, loc))
                    }
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
                decorators,
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

                let mut func_node = self.ctx.builder().function(name, params, vec![body_id]);

                for dec in decorators.iter().rev() {
                    let dec_node = self.convert_expression(dec);
                    func_node = self.ctx.builder().call(dec_node, vec![func_node], loc.clone());
                }

                Some(self.ctx.builder().assign(name, func_node, loc))
            }
            Statement::AsyncFunctionDef {
                decorators,
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

                let mut func_node = self.ctx.builder().function(name, params, vec![body_id]);
                func_node = self.ctx.builder().extension("async", vec![func_node], loc.clone());

                for dec in decorators.iter().rev() {
                    let dec_node = self.convert_expression(dec);
                    func_node = self.ctx.builder().call(dec_node, vec![func_node], loc.clone());
                }

                Some(self.ctx.builder().assign(name, func_node, loc))
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
            Statement::AsyncFor {
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
                Some(self.ctx.builder().extension("async_foreach", vec![target_node, iter_node, body_id], loc))
            }
            Statement::Pass => Some(self.ctx.builder().constant(0, loc)),
            Statement::Break => Some(self.ctx.builder().extension("break", vec![], loc)),
            Statement::Continue => Some(self.ctx.builder().extension("continue", vec![], loc)),
            Statement::Raise { exc, cause } => {
                let exc_node = exc
                    .as_ref()
                    .map(|e| self.convert_expression(e))
                    .unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                let cause_node = cause
                    .as_ref()
                    .map(|e| self.convert_expression(e))
                    .unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                Some(self.ctx.builder().extension("raise", vec![exc_node, cause_node], loc))
            }
            Statement::Assert { test, msg } => {
                let test_node = self.convert_expression(test);
                let msg_node = msg
                    .as_ref()
                    .map(|e| self.convert_expression(e))
                    .unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                Some(self.ctx.builder().extension("assert", vec![test_node, msg_node], loc))
            }
            Statement::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                let body_items = body.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                let body_id = self.ctx.builder().block(body_items, loc.clone());

                let mut handler_nodes = Vec::new();
                for handler in handlers {
                    self.ctx.scopes.push_scope();
                    let name_node = if let Some(name) = &handler.name {
                        let var = self.ctx.scopes.declare_variable(name);
                        self.ctx.builder().symbol(&var, loc.clone())
                    } else {
                        self.ctx.builder().constant(0, loc.clone())
                    };

                    let type_node = handler
                        .type_
                        .as_ref()
                        .map(|e| self.convert_expression(e))
                        .unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));

                    let handler_body_items = handler.body.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                    let handler_body_id = self.ctx.builder().block(handler_body_items, loc.clone());

                    handler_nodes.push(self.ctx.builder().extension("except", vec![type_node, name_node, handler_body_id], loc.clone()));
                    self.ctx.scopes.pop_scope();
                }
                let handlers_id = self.ctx.builder().block(handler_nodes, loc.clone());

                let orelse_items = orelse.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                let orelse_id = self.ctx.builder().block(orelse_items, loc.clone());

                let final_items = finalbody.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                let final_id = self.ctx.builder().block(final_items, loc.clone());

                Some(self.ctx.builder().extension("try", vec![body_id, handlers_id, orelse_id, final_id], loc))
            }
            Statement::With { items, body } => {
                self.ctx.scopes.push_scope();
                let mut item_nodes = Vec::new();
                for item in items {
                    let ctx_expr = self.convert_expression(&item.context_expr);
                    let var_node = if let Some(v) = &item.optional_vars {
                        if let Expression::Name(name) = v {
                            let var = self.ctx.scopes.declare_variable(name);
                            self.ctx.builder().symbol(&var, loc.clone())
                        } else {
                            self.convert_expression(v)
                        }
                    } else {
                        self.ctx.builder().constant(0, loc.clone())
                    };
                    item_nodes.push(self.ctx.builder().extension("with_item", vec![ctx_expr, var_node], loc.clone()));
                }
                let items_id = self.ctx.builder().block(item_nodes, loc.clone());

                let body_items = body.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                let body_id = self.ctx.builder().block(body_items, loc.clone());

                let result = Some(self.ctx.builder().extension("with", vec![items_id, body_id], loc));
                self.ctx.scopes.pop_scope();
                result
            }
            Statement::AsyncWith { items, body } => {
                self.ctx.scopes.push_scope();
                let mut item_nodes = Vec::new();
                for item in items {
                    let ctx_expr = self.convert_expression(&item.context_expr);
                    let var_node = if let Some(v) = &item.optional_vars {
                        if let Expression::Name(name) = v {
                            let var = self.ctx.scopes.declare_variable(name);
                            self.ctx.builder().symbol(&var, loc.clone())
                        } else {
                            self.convert_expression(v)
                        }
                    } else {
                        self.ctx.builder().constant(0, loc.clone())
                    };
                    item_nodes.push(self.ctx.builder().extension("with_item", vec![ctx_expr, var_node], loc.clone()));
                }
                let items_id = self.ctx.builder().block(item_nodes, loc.clone());

                let body_items = body.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                let body_id = self.ctx.builder().block(body_items, loc.clone());

                let result = Some(self.ctx.builder().extension("async_with", vec![items_id, body_id], loc));
                self.ctx.scopes.pop_scope();
                result
            }
            Statement::Match { subject, cases } => {
                let subject_node = self.convert_expression(subject);
                let mut case_nodes = Vec::new();
                for case in cases {
                    let pattern_node = self.convert_pattern(&case.pattern);
                    let guard_node = case
                        .guard
                        .as_ref()
                        .map(|g| self.convert_expression(g))
                        .unwrap_or_else(|| self.ctx.builder().constant(1, loc.clone()));
                    let body_items = case.body.iter().filter_map(|s| self.convert_statement(s)).collect::<Vec<_>>();
                    let body_id = self.ctx.builder().block(body_items, loc.clone());
                    case_nodes.push(self.ctx.builder().extension(
                        "match_case",
                        vec![pattern_node, guard_node, body_id],
                        loc.clone(),
                    ));
                }
                let cases_id = self.ctx.builder().block(case_nodes, loc.clone());
                Some(self.ctx.builder().extension("match", vec![subject_node, cases_id], loc))
            }
            Statement::Import { names } => {
                let mut last = None;
                for name in names {
                    let module_name = self.ctx.builder().constant(name.name.clone(), loc.clone());
                    let module_val = self.ctx.builder().extension("import", vec![module_name], loc.clone());
                    let target_name = name.asname.as_ref().unwrap_or(&name.name);
                    let target_id = self.ctx.scopes.declare_variable(target_name);
                    last = Some(self.ctx.builder().assign(&target_id, module_val, loc.clone()));
                }
                last
            }
            Statement::ImportFrom { module, names } => {
                let mut last = None;
                let module_str = module.clone().unwrap_or_default();
                let module_name = self.ctx.builder().constant(module_str, loc.clone());
                for name in names {
                    let member_name = self.ctx.builder().constant(name.name.clone(), loc.clone());
                    let val = self.ctx.builder().extension("import_from", vec![module_name, member_name], loc.clone());
                    let target_name = name.asname.as_ref().unwrap_or(&name.name);
                    let target_id = self.ctx.scopes.declare_variable(target_name);
                    last = Some(self.ctx.builder().assign(&target_id, val, loc.clone()));
                }
                last
            }
            Statement::Global { names } => {
                for name in names {
                    self.ctx.scopes.declare_global(name);
                }
                Some(self.ctx.builder().constant(0, loc))
            }
            Statement::Nonlocal { names } => {
                for name in names {
                    self.ctx.scopes.declare_nonlocal(name);
                }
                Some(self.ctx.builder().constant(0, loc))
            }
            Statement::ClassDef { decorators, name, bases, body } => {
                let base_nodes = bases.iter().map(|b| self.convert_expression(b)).collect::<Vec<_>>();
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.convert_statement(s) {
                        body_items.push(node);
                    }
                }
                let body_id = self.ctx.builder().block(body_items, loc.clone());
                let bases_id = self.ctx.builder().extension("bases", base_nodes, loc.clone());
                let mut class_node = self.ctx.builder().extension(
                    "class_def",
                    vec![self.ctx.builder().symbol(name, loc.clone()), bases_id, body_id],
                    loc.clone(),
                );

                for dec in decorators.iter().rev() {
                    let dec_node = self.convert_expression(dec);
                    class_node = self.ctx.builder().call(dec_node, vec![class_node], loc.clone());
                }

                Some(self.ctx.builder().assign(name, class_node, loc))
            }
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
                Literal::Bytes(_) => self.ctx.builder().extension("bytes", vec![], loc),
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
            Expression::Dict { keys, values } => {
                let mut nodes = Vec::new();
                for (k, v) in keys.iter().zip(values.iter()) {
                    let k_node = k
                        .as_ref()
                        .map(|e| self.convert_expression(e))
                        .unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                    let v_node = self.convert_expression(v);
                    nodes.push(self.ctx.builder().extension("dict_item", vec![k_node, v_node], loc.clone()));
                }
                self.ctx.builder().extension("dict", nodes, loc)
            }
            Expression::Set { elts } => {
                let nodes = elts.iter().map(|e| self.convert_expression(e)).collect();
                self.ctx.builder().extension("set", nodes, loc)
            }
            Expression::ListComp { elt, generators } => {
                let elt_node = self.convert_expression(elt);
                let mut nodes = vec![elt_node];
                for gen in generators {
                    nodes.push(self.convert_comprehension(gen));
                }
                self.ctx.builder().extension("list_comp", nodes, loc)
            }
            Expression::SetComp { elt, generators } => {
                let elt_node = self.convert_expression(elt);
                let mut nodes = vec![elt_node];
                for gen in generators {
                    nodes.push(self.convert_comprehension(gen));
                }
                self.ctx.builder().extension("set_comp", nodes, loc)
            }
            Expression::DictComp { key, value, generators } => {
                let key_node = self.convert_expression(key);
                let val_node = self.convert_expression(value);
                let mut nodes = vec![key_node, val_node];
                for gen in generators {
                    nodes.push(self.convert_comprehension(gen));
                }
                self.ctx.builder().extension("dict_comp", nodes, loc)
            }
            Expression::GeneratorExp { elt, generators } => {
                let elt_node = self.convert_expression(elt);
                let mut nodes = vec![elt_node];
                for gen in generators {
                    nodes.push(self.convert_comprehension(gen));
                }
                self.ctx.builder().extension("generator_exp", nodes, loc)
            }
            Expression::Slice { lower, upper, step } => {
                let lower_node = lower.as_ref().map(|e| self.convert_expression(e)).unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                let upper_node = upper.as_ref().map(|e| self.convert_expression(e)).unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                let step_node = step.as_ref().map(|e| self.convert_expression(e)).unwrap_or_else(|| self.ctx.builder().constant(0, loc.clone()));
                self.ctx.builder().extension("slice", vec![lower_node, upper_node, step_node], loc)
            }
            Expression::JoinedStr { values } => {
                let nodes = values.iter().map(|v| self.convert_expression(v)).collect();
                self.ctx.builder().extension("fstring", nodes, loc)
            }
            Expression::FormattedValue { value, .. } => {
                let val_node = self.convert_expression(value);
                self.ctx.builder().extension("formatted_value", vec![val_node], loc)
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
            Expression::Attribute { value, attr } => {
                let value_node = self.convert_expression(value);
                let attr_id = self.ctx.builder().symbol(attr, loc.clone());
                self.ctx.builder().extension("get_field", vec![value_node, attr_id], loc)
            }
            Expression::Await(expr) => {
                let node = self.convert_expression(expr);
                self.ctx.builder().extension("await", vec![node], loc)
            }
            Expression::Yield(expr) => {
                let node = if let Some(e) = expr {
                    self.convert_expression(e)
                } else {
                    self.ctx.builder().constant(0, loc.clone())
                };
                self.ctx.builder().extension("yield", vec![node], loc)
            }
            Expression::YieldFrom(expr) => {
                let node = self.convert_expression(expr);
                self.ctx.builder().extension("yield_from", vec![node], loc)
            }
            Expression::Subscript { value, slice } => {
                let value_node = self.convert_expression(value);
                let slice_node = self.convert_expression(slice);
                self.ctx.builder().extension("subscript", vec![value_node, slice_node], loc)
            }
            Expression::Lambda { args, body } => {
                self.ctx.scopes.push_scope();
                let params = args.iter().map(|p| self.ctx.scopes.declare_variable(&p.name)).collect();
                let body_node = self.convert_expression(body);
                let body_id = self.ctx.builder().block(vec![body_node], loc.clone());
                self.ctx.scopes.pop_scope();
                self.ctx.builder().function("lambda", params, vec![body_id])
            }
            Expression::IfExp { test, body, orelse } => {
                let cond = self.convert_expression(test);
                let then_node = self.convert_expression(body);
                let else_node = self.convert_expression(orelse);
                self.ctx.builder().extension("if_exp", vec![cond, then_node, else_node], loc)
            }
            Expression::Starred { value, is_double } => {
                let val_node = self.convert_expression(value);
                let op = if *is_double { "double_starred" } else { "starred" };
                self.ctx.builder().extension(op, vec![val_node], loc)
            }
        }
    }

    fn convert_comprehension(&mut self, gen: &oak_python::ast::Comprehension) -> Id {
        let loc = Loc::default();
        let target_node = self.convert_expression(&gen.target);
        let iter_node = self.convert_expression(&gen.iter);
        let ifs_nodes: Vec<Id> = gen.ifs.iter().map(|i| self.convert_expression(i)).collect();
        let ifs_block = self.ctx.builder().block(ifs_nodes, loc.clone());
        let async_node = self.ctx.builder().constant(if gen.is_async { 1 } else { 0 }, loc.clone());
        self.ctx.builder().extension("comprehension", vec![target_node, iter_node, ifs_block, async_node], loc)
    }

    fn convert_pattern(&mut self, pattern: &oak_python::ast::Pattern) -> Id {
        let loc = Loc::default();
        match pattern {
            oak_python::ast::Pattern::Value(expr) => {
                let node = self.convert_expression(expr);
                self.ctx.builder().extension("pattern_value", vec![node], loc)
            }
            oak_python::ast::Pattern::Wildcard => self.ctx.builder().extension("pattern_wildcard", vec![], loc),
            oak_python::ast::Pattern::As { pattern, name } => {
                let inner = if let Some(p) = pattern {
                    self.convert_pattern(p)
                } else {
                    self.ctx.builder().extension("pattern_wildcard", vec![], loc.clone())
                };
                let name_id = self.ctx.scopes.declare_variable(name);
                let name_node = self.ctx.builder().symbol(&name_id, loc.clone());
                self.ctx.builder().extension("pattern_as", vec![inner, name_node], loc)
            }
            oak_python::ast::Pattern::Sequence(patterns) => {
                let nodes = patterns.iter().map(|p| self.convert_pattern(p)).collect();
                self.ctx.builder().extension("pattern_sequence", nodes, loc)
            }
            oak_python::ast::Pattern::Mapping { keys, patterns } => {
                let mut nodes = Vec::new();
                for (k, p) in keys.iter().zip(patterns.iter()) {
                    let k_node = self.convert_expression(k);
                    let p_node = self.convert_pattern(p);
                    nodes.push(self.ctx.builder().extension("pattern_mapping_item", vec![k_node, p_node], loc.clone()));
                }
                self.ctx.builder().extension("pattern_mapping", nodes, loc)
            }
            oak_python::ast::Pattern::Class { cls, patterns, keywords, keyword_patterns } => {
                let cls_node = self.convert_expression(cls);
                let mut nodes = vec![cls_node];
                for p in patterns {
                    nodes.push(self.convert_pattern(p));
                }
                for (k, p) in keywords.iter().zip(keyword_patterns.iter()) {
                    let k_node = self.ctx.builder().constant(k.clone(), loc.clone());
                    let p_node = self.convert_pattern(p);
                    nodes.push(self.ctx.builder().extension("pattern_class_keyword", vec![k_node, p_node], loc.clone()));
                }
                self.ctx.builder().extension("pattern_class", nodes, loc)
            }
            oak_python::ast::Pattern::Or(patterns) => {
                let nodes = patterns.iter().map(|p| self.convert_pattern(p)).collect();
                self.ctx.builder().extension("pattern_or", nodes, loc)
            }
        }
    }
}
