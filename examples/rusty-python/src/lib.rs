//! Rusty Python 语言前端
//!
//! 这个库提供了 Rusty Python 语言的词法分析、语法分析和 Gaia 翻译功能。

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::Parser;
use oak_python::ast::{Expression, Literal, PythonRoot, Statement};

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

    /// 词法分析（仅用于测试）
    pub fn tokenize(&self, _source: &str) -> Result<Vec<String>, NyarError> {
        // TODO: 实现真正的词法分析导出
        Ok(vec!["dummy_token".to_string()])
    }

    /// 编译到 Gaia 程序
    pub fn compile_to_gaia(&self, source: &str) -> Result<gaia_assembler::program::GaiaModule, NyarError> {
        let ast = self.parse(source)?;
        let tree = self.lower(&ast)?;
        let mut translator = codegen::GaiaTranslator::new();
        translator.generate_from_tree(&tree).map_err(|e| NyarError::Compile(format!("{:?}", e)))
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

    fn lower(&self, ast: &PythonRoot) -> Result<IKunTree, NyarError> {
        let mut items = Vec::new();
        for stmt in &ast.program.statements {
            if let Some(node) = self.lower_statement(stmt) {
                items.push(node);
            }
        }
        Ok(IKunTree::Module("rusty-python-program".to_string(), items))
    }
}

impl RustyPythonFrontend {
    fn lower_statement(&self, stmt: &Statement) -> Option<IKunTree> {
        match stmt {
            Statement::Assignment { target, value } => {
                let target_node = self.lower_expression(target);
                let value_node = self.lower_expression(value);
                Some(IKunTree::StateUpdate(
                    Box::new(target_node),
                    Box::new(value_node),
                ))
            }
            Statement::AugmentedAssignment {
                target,
                operator,
                value,
            } => {
                let target_node = self.lower_expression(target);
                let value_node = self.lower_expression(value);
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
                let result_node =
                    IKunTree::Extension(op_name.to_string(), vec![target_node.clone(), value_node]);
                Some(IKunTree::StateUpdate(
                    Box::new(target_node),
                    Box::new(result_node),
                ))
            }
            Statement::Expression(expr) => Some(self.lower_expression(expr)),
            Statement::FunctionDef {
                name,
                parameters,
                body,
                ..
            } => {
                let params = parameters.iter().map(|p| p.name.clone()).collect();
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.lower_statement(s) {
                        body_items.push(node);
                    }
                }
                Some(IKunTree::StateUpdate(
                    Box::new(IKunTree::Symbol(name.clone())),
                    Box::new(IKunTree::Lambda(
                        params,
                        Box::new(IKunTree::Seq(body_items)),
                    )),
                ))
            }
            Statement::Return(expr) => {
                let val = expr
                    .as_ref()
                    .map(|e| self.lower_expression(e))
                    .unwrap_or(IKunTree::Constant(0));
                Some(IKunTree::Apply(
                    Box::new(IKunTree::Symbol("return".to_string())),
                    vec![val],
                ))
            }
            Statement::If { test, body, orelse } => {
                let cond = self.lower_expression(test);
                let mut then_items = Vec::new();
                for s in body {
                    if let Some(node) = self.lower_statement(s) {
                        then_items.push(node);
                    }
                }
                let mut else_items = Vec::new();
                for s in orelse {
                    if let Some(node) = self.lower_statement(s) {
                        else_items.push(node);
                    }
                }
                Some(IKunTree::Choice(
                    Box::new(cond),
                    Box::new(IKunTree::Seq(then_items)),
                    Box::new(IKunTree::Seq(else_items)),
                ))
            }
            Statement::While { test, body, .. } => {
                let cond = self.lower_expression(test);
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.lower_statement(s) {
                        body_items.push(node);
                    }
                }
                Some(IKunTree::Repeat(
                    Box::new(cond),
                    Box::new(IKunTree::Seq(body_items)),
                ))
            }
            Statement::For {
                target,
                iter,
                body,
                ..
            } => {
                let target_node = self.lower_expression(target);
                let iter_node = self.lower_expression(iter);
                let mut body_items = Vec::new();
                for s in body {
                    if let Some(node) = self.lower_statement(s) {
                        body_items.push(node);
                    }
                }
                // Map to a custom extension for for-each
                Some(IKunTree::Extension(
                    "foreach".to_string(),
                    vec![target_node, iter_node, IKunTree::Seq(body_items)],
                ))
            }
            Statement::Pass => Some(IKunTree::Seq(vec![])),
            Statement::Break => Some(IKunTree::Apply(
                Box::new(IKunTree::Symbol("break".to_string())),
                vec![],
            )),
            Statement::Continue => Some(IKunTree::Apply(
                Box::new(IKunTree::Symbol("continue".to_string())),
                vec![],
            )),
            _ => None,
        }
    }

    fn lower_expression(&self, expr: &Expression) -> IKunTree {
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Integer(i) => IKunTree::Constant(*i),
                Literal::Float(f) => IKunTree::FloatConstant(f.to_bits()),
                Literal::String(s) => IKunTree::StringConstant(s.clone()),
                Literal::Boolean(b) => IKunTree::BooleanConstant(*b),
                Literal::None => IKunTree::Constant(0),
            },
            Expression::Name(name) => IKunTree::Symbol(name.clone()),
            Expression::BinaryOp {
                left,
                operator,
                right,
            } => {
                let left_node = self.lower_expression(left);
                let right_node = self.lower_expression(right);
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
                IKunTree::Extension(op_name.to_string(), vec![left_node, right_node])
            }
            Expression::UnaryOp { operator, operand } => {
                let operand_node = self.lower_expression(operand);
                let op_name = match operator {
                    oak_python::ast::UnaryOperator::Invert => "invert",
                    oak_python::ast::UnaryOperator::Not => "not",
                    oak_python::ast::UnaryOperator::UAdd => "uadd",
                    oak_python::ast::UnaryOperator::USub => "usub",
                };
                IKunTree::Extension(op_name.to_string(), vec![operand_node])
            }
            Expression::BoolOp { operator, values } => {
                let op_name = match operator {
                    oak_python::ast::BoolOperator::And => "and",
                    oak_python::ast::BoolOperator::Or => "or",
                };
                let nodes = values.iter().map(|v| self.lower_expression(v)).collect();
                IKunTree::Extension(op_name.to_string(), nodes)
            }
            Expression::List { elts } => {
                let nodes = elts.iter().map(|e| self.lower_expression(e)).collect();
                IKunTree::Extension("list".to_string(), nodes)
            }
            Expression::Tuple { elts } => {
                let nodes = elts.iter().map(|e| self.lower_expression(e)).collect();
                IKunTree::Extension("tuple".to_string(), nodes)
            }
            Expression::Compare {
                left,
                ops,
                comparators,
            } => {
                let left_node = self.lower_expression(left);
                let right_node = self.lower_expression(&comparators[0]);
                let op_name = match ops[0] {
                    oak_python::ast::CompareOperator::Eq => "eq",
                    oak_python::ast::CompareOperator::NotEq => "noteq",
                    oak_python::ast::CompareOperator::Lt => "lt",
                    oak_python::ast::CompareOperator::LtE => "lte",
                    oak_python::ast::CompareOperator::Gt => "gt",
                    oak_python::ast::CompareOperator::GtE => "gte",
                    _ => "unknown",
                };
                IKunTree::Extension(op_name.to_string(), vec![left_node, right_node])
            }
            Expression::Call { func, args, .. } => {
                let func_node = self.lower_expression(func);
                let args_nodes: Vec<IKunTree> =
                    args.iter().map(|a| self.lower_expression(a)).collect();

                // 特殊处理 print
                if let IKunTree::Symbol(ref name) = func_node {
                    if name == "print" {
                        // Python's print defaults to newline.
                        // Map to a standard cross-language call that all backends should handle.
                        return IKunTree::CrossLangCall(
                            "nyar".to_string(),
                            "std::io::println".to_string(),
                            args_nodes,
                        );
                    }
                }

                IKunTree::Apply(Box::new(func_node), args_nodes)
            }
            _ => IKunTree::Constant(0),
        }
    }

    /// 编译到 Python 字节码 (.pyc)
    pub fn compile_to_pyc(&self, source: &str) -> Result<Vec<u8>, NyarError> {
        use crate::pyc_codegen::{emit_pyc, PycTranslator};
        let ast = self.parse(source)?;
        let tree = self.lower(&ast)?;
        let mut translator = PycTranslator::new("program.py", "<module>");
        let program = translator.translate_from_tree(&tree);
        emit_pyc(&program).map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }
}
