#![feature(new_range_api)]
//! Mini C 解释器
//!
//! 基于 Oaks (前端), Chomsky (优化), Gaia (后端) 和 Nyar VM (运行时) 架构实现。

pub mod frontend;
pub mod optimizer;
pub mod runtime;

use chomsky_extract::IKunExtractor;
use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder};
use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_c::{ast, CBuilder, CLanguage, CRoot};
use oak_core::source::SourceText;
use std::ops::Range;

/// Mini C 前端实现
#[derive(Default)]
pub struct MiniCFrontend {
    language: CLanguage,
}

impl MiniCFrontend {
    /// 创建一个新的 Mini C 前端
    pub fn new() -> Self {
        Self {
            language: CLanguage::default(),
        }
    }
}

impl NyarFrontend for MiniCFrontend {
    type Language = CLanguage;

    fn parse(&self, source: &str) -> Result<CRoot, NyarError> {
        use oak_core::Builder;
        let builder = CBuilder::new(&self.language);
        let mut session = oak_core::parser::session::ParseSession::<CLanguage>::default();
        let source_text = SourceText::new(source.to_string());
        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower(&self, ast: &CRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        let mut converter = UirConverter::new(&mut builder, 1);

        let root_id = converter.convert_root(ast);

        let extractor = IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        let tree = extractor.extract(root_id);
        Ok(tree)
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

    fn convert_root(&mut self, root: &CRoot) -> Id {
        let mut items = Vec::new();
        for decl in &root.translation_unit.external_declarations {
            items.push(self.convert_external_declaration(decl));
        }
        self.builder.module("main", items)
    }

    fn convert_external_declaration(&mut self, decl: &ast::ExternalDeclaration) -> Id {
        match decl {
            ast::ExternalDeclaration::FunctionDefinition(func) => {
                self.convert_function_definition(func)
            }
            ast::ExternalDeclaration::Declaration(decl) => self.convert_declaration(decl),
        }
    }

    fn convert_function_definition(&mut self, func: &ast::FunctionDefinition) -> Id {
        let loc = self.to_loc(func.span.clone().into());
        let name = self.get_declarator_name(&func.declarator);

        let mut params = Vec::new();
        if let ast::DirectDeclarator::Function {
            parameter_type_list,
            ..
        } = &func.declarator.direct_declarator
        {
            if let Some(list) = parameter_type_list {
                for param in &list.parameter_list {
                    if let Some(decl) = &param.declarator {
                        params.push(self.get_declarator_name(decl));
                    }
                }
            }
        }

        let mut body_ids = Vec::new();
        for item in &func.compound_statement.block_items {
            body_ids.push(self.convert_block_item(item));
        }

        if name == "main" {
            return self.builder.block(body_ids, loc);
        }

        let lambda = self.builder.function(&name, params, body_ids);
        self.builder.assign(&name, lambda, loc)
    }

    fn convert_declaration(&mut self, decl: &ast::Declaration) -> Id {
        let loc = self.to_loc(decl.span.clone().into());
        let mut ids = Vec::new();
        for init in &decl.init_declarators {
            let name = self.get_declarator_name(&init.declarator);
            let value = if let Some(init_val) = &init.initializer {
                self.convert_initializer(init_val)
            } else {
                self.builder.constant(0, loc.clone())
            };
            ids.push(self.builder.assign(&name, value, loc.clone()));
        }
        if ids.len() == 1 {
            ids[0]
        } else {
            self.builder.block(ids, loc)
        }
    }

    fn convert_block_item(&mut self, item: &ast::BlockItem) -> Id {
        match item {
            ast::BlockItem::Declaration(decl) => self.convert_declaration(decl),
            ast::BlockItem::Statement(stmt) => self.convert_statement(stmt),
        }
    }

    fn convert_statement(&mut self, stmt: &ast::Statement) -> Id {
        let loc = self.to_loc(stmt.span().into());
        match stmt {
            ast::Statement::Compound(comp) => {
                let mut ids = Vec::new();
                for item in &comp.block_items {
                    ids.push(self.convert_block_item(item));
                }
                self.builder.block(ids, loc)
            }
            ast::Statement::Expression(expr_stmt) => {
                if let Some(expr) = &expr_stmt.expression {
                    self.convert_expression(expr)
                } else {
                    self.builder
                        .constant(0, self.to_loc(expr_stmt.span.clone().into()))
                }
            }
            ast::Statement::Selection(sel) => match sel {
                ast::SelectionStatement::If {
                    condition,
                    then_statement,
                    else_statement,
                    span,
                } => {
                    let cond = self.convert_expression(condition);
                    let then_id = self.convert_statement(then_statement);
                    let else_id = if let Some(e) = else_statement {
                        self.convert_statement(e)
                    } else {
                        self.builder.constant(0, loc.clone())
                    };
                    self.builder.extension(
                        "if",
                        vec![cond, then_id, else_id],
                        self.to_loc(span.clone().into()),
                    )
                }
                _ => self.builder.constant(0, loc),
            },
            ast::Statement::Iteration(iter) => match iter {
                ast::IterationStatement::While {
                    condition,
                    statement,
                    span,
                } => {
                    let cond = self.convert_expression(condition);
                    let body = self.convert_statement(statement);
                    self.builder.extension(
                        "while",
                        vec![cond, body],
                        self.to_loc(span.clone().into()),
                    )
                }
                ast::IterationStatement::DoWhile {
                    statement,
                    condition,
                    span,
                } => {
                    let body = self.convert_statement(statement);
                    let cond = self.convert_expression(condition);
                    self.builder.extension(
                        "do_while",
                        vec![body, cond],
                        self.to_loc(span.clone().into()),
                    )
                }
                ast::IterationStatement::For {
                    init,
                    condition,
                    update,
                    statement,
                    span,
                } => {
                    let i = if let Some(e) = init {
                        self.convert_expression(e)
                    } else {
                        self.builder.constant(0, loc.clone())
                    };
                    let c = if let Some(e) = condition {
                        self.convert_expression(e)
                    } else {
                        self.builder.constant(1, loc.clone())
                    };
                    let u = if let Some(e) = update {
                        self.convert_expression(e)
                    } else {
                        self.builder.constant(0, loc.clone())
                    };
                    let b = self.convert_statement(statement);
                    self.builder.extension(
                        "for",
                        vec![i, c, u, b],
                        self.to_loc(span.clone().into()),
                    )
                }
            },
            ast::Statement::Jump(jump) => match jump {
                ast::JumpStatement::Return(expression, _) => {
                    let val = if let Some(e) = expression {
                        self.convert_expression(e)
                    } else {
                        self.builder.constant(0, loc.clone())
                    };
                    self.builder.return_(val, loc)
                }
                ast::JumpStatement::Break(span) => {
                    self.builder
                        .extension("break", vec![], self.to_loc(span.clone().into()))
                }
                ast::JumpStatement::Continue(span) => {
                    self.builder
                        .extension("continue", vec![], self.to_loc(span.clone().into()))
                }
                _ => self.builder.constant(0, loc),
            },
            _ => self.builder.constant(0, loc),
        }
    }

    fn convert_expression(&mut self, expr: &ast::Expression) -> Id {
        let loc = self.to_loc(expr.span.clone().into());
        match &*expr.kind {
            ast::ExpressionKind::Constant(c, _) => match c {
                ast::Constant::Integer(val, _) => self.builder.constant(*val, loc),
                _ => self.builder.constant(0, loc),
            },
            ast::ExpressionKind::Identifier(name, _) => self.builder.symbol(name, loc),
            ast::ExpressionKind::Binary {
                left,
                operator,
                right,
                ..
            } => {
                let l = self.convert_expression(left);
                let r = self.convert_expression(right);
                let op = format!("{:?}", operator).to_lowercase();
                self.builder.binary_op(&op, l, r, loc)
            }
            ast::ExpressionKind::Unary {
                operator, operand, ..
            } => {
                let arg = self.convert_expression(operand);
                let op = format!("{:?}", operator).to_lowercase();
                self.builder.extension(&op, vec![arg], loc)
            }
            ast::ExpressionKind::Assignment {
                left,
                operator,
                right,
                ..
            } => {
                let r = self.convert_expression(right);
                let l = self.convert_expression(left);
                let op = format!("{:?}", operator).to_lowercase();
                if op == "assign" {
                    self.builder.assign_to_id(l, r, loc)
                } else {
                    let base_op = op.replace("assign", "");
                    let value = self.builder.binary_op(&base_op, l, r, loc.clone());
                    self.builder.assign_to_id(l, value, loc)
                }
            }
            ast::ExpressionKind::FunctionCall {
                function,
                arguments,
                ..
            } => {
                let func_id = self.convert_expression(function);
                let mut args = Vec::new();
                for arg in arguments {
                    args.push(self.convert_expression(arg));
                }

                if let ast::ExpressionKind::Identifier(name, _) = &*function.kind {
                    if name == "printf" {
                        let symbol = self.builder.symbol("System.Console.WriteLine", loc.clone());
                        return self.builder.call(symbol, args, loc);
                    }
                }

                self.builder.call(func_id, args, loc)
            }
            _ => self.builder.constant(0, loc),
        }
    }

    fn convert_initializer(&mut self, init: &ast::Initializer) -> Id {
        match init {
            ast::Initializer::AssignmentExpression(expr) => self.convert_expression(expr),
            ast::Initializer::InitializerList(list, span) => {
                let loc = self.to_loc(span.clone().into());
                let mut ids = Vec::new();
                for item in list {
                    ids.push(self.convert_initializer(item));
                }
                self.builder.extension("array", ids, loc)
            }
        }
    }

    fn get_declarator_name(&self, decl: &ast::Declarator) -> String {
        self.get_direct_declarator_name(&decl.direct_declarator)
    }

    fn get_direct_declarator_name(&self, decl: &ast::DirectDeclarator) -> String {
        match decl {
            ast::DirectDeclarator::Identifier(name, _) => name.clone(),
            ast::DirectDeclarator::Declarator(decl) => self.get_declarator_name(decl),
            ast::DirectDeclarator::Array { declarator, .. } => {
                self.get_direct_declarator_name(declarator)
            }
            ast::DirectDeclarator::Function { declarator, .. } => {
                self.get_direct_declarator_name(declarator)
            }
        }
    }
}
