use chomsky_source::Loc;
use crate::GoBuilder;
use chomsky_uir::{IKunTree, IntentBuilder, Id};
use oak_core::parser::ParseSession;
use oak_core::{SourceText, RedNode, RedTree, Builder};
use oak_go::{ast, GoLanguage, GoRoot, GoSyntaxKind};

#[derive(Default)]
pub struct RustyGoFrontend;

impl nyar_types::NyarFrontend for RustyGoFrontend {
    type Language = GoLanguage;

    fn parse(&self, source: &str) -> Result<GoRoot, nyar_types::NyarError> {
        let language = GoLanguage::default();
        let builder = GoBuilder::new(&language);
        let source_text = SourceText::new(source.to_string());
        
        let mut session = ParseSession::new(1024);
        let output = builder.build(&source_text, &[], &mut session);

        output.result.map_err(|e| nyar_types::NyarError::Compile(format!("Build error: {:?}", e)))
    }

    fn lower(&self, ast: &GoRoot) -> Result<IKunTree, nyar_types::NyarError> {
        let mut items = vec![];

        for decl in &ast.declarations {
            match decl {
                ast::Declaration::Function(func) => {
                    let mut params = vec![];
                    for p in &func.params {
                        params.push(p.name.clone());
                    }
                    let body = self.lower_block(&func.body)?;

                    // 为 main 函数创建导出
                    if func.name == "main" {
                        items.push(IKunTree::Export(
                            "main".to_string(),
                            Box::new(IKunTree::Lambda(params, Box::new(body))),
                        ));
                    } else {
                        items.push(IKunTree::Export(
                            func.name.clone(),
                            Box::new(IKunTree::Lambda(params, Box::new(body))),
                        ));
                    }
                }
                ast::Declaration::Variable(var) => {
                    let val = if let Some(v) = &var.value {
                        self.lower_expression(v)?
                    } else {
                        IKunTree::Constant(0)
                    };
                    items.push(IKunTree::Export(
                        var.name.clone(),
                        Box::new(val),
                    ));
                }
                ast::Declaration::Const(c) => {
                    let val = self.lower_expression(&c.value)?;
                    items.push(IKunTree::Export(
                        c.name.clone(),
                        Box::new(val),
                    ));
                }
                _ => {}
            }
        }

        Ok(IKunTree::Module("rusty-go-program".to_string(), items))
    }
}

impl RustyGoFrontend {
    pub fn new() -> Self {
        Self
    }

    fn lower_block(&self, block: &ast::Block) -> Result<IKunTree, nyar_types::NyarError> {
        let mut items = vec![];
        for stmt in &block.statements {
            items.push(self.lower_statement(stmt)?);
        }
        Ok(IKunTree::Seq(items))
    }

    fn lower_statement(&self, stmt: &ast::Statement) -> Result<IKunTree, nyar_types::NyarError> {
        match stmt {
            ast::Statement::Expression(expr) => self.lower_expression(expr),
            ast::Statement::Assignment { target, value, .. } => {
                let val = self.lower_expression(value)?;
                Ok(IKunTree::StateUpdate(
                    Box::new(IKunTree::Symbol(target.clone())),
                    Box::new(val),
                ))
            }
            ast::Statement::Return { value, .. } => {
                let val = if let Some(v) = value {
                    self.lower_expression(v)?
                } else {
                    IKunTree::Seq(vec![])
                };
                Ok(IKunTree::Return(Box::new(val)))
            }
            ast::Statement::If { condition, then_block, else_block, .. } => {
                let cond = self.lower_expression(condition)?;
                let then = self.lower_block(then_block)?;
                let els = if let Some(eb) = else_block {
                    self.lower_block(eb)?
                } else {
                    IKunTree::Seq(vec![])
                };
                Ok(IKunTree::Choice(Box::new(cond), Box::new(then), Box::new(els)))
            }
            ast::Statement::For { init, condition, post, body, .. } => {
                let mut stmts = vec![];
                if let Some(i) = init {
                    stmts.push(self.lower_statement(i)?);
                }

                let cond = if let Some(c) = condition {
                    self.lower_expression(c)?
                } else {
                    IKunTree::BooleanConstant(true)
                };

                let mut body_stmts = vec![self.lower_block(body)?];
                if let Some(p) = post {
                    body_stmts.push(self.lower_statement(p)?);
                }

                stmts.push(IKunTree::Repeat(Box::new(cond), Box::new(IKunTree::Seq(body_stmts))));
                Ok(IKunTree::Seq(stmts))
            }
        }
    }

    fn lower_expression(&self, expr: &ast::Expression) -> Result<IKunTree, nyar_types::NyarError> {
        match expr {
            ast::Expression::Identifier { name, .. } => Ok(IKunTree::Symbol(name.clone())),
            ast::Expression::Literal { value, .. } => {
                if value.starts_with('"') && value.ends_with('"') {
                    let s = &value[1..value.len() - 1];
                    Ok(IKunTree::StringConstant(s.to_string()))
                } else if value == "true" {
                    Ok(IKunTree::BooleanConstant(true))
                } else if value == "false" {
                    Ok(IKunTree::BooleanConstant(false))
                } else if let Ok(n) = value.parse::<i64>() {
                    Ok(IKunTree::Constant(n))
                } else {
                    Ok(IKunTree::StringConstant(value.clone()))
                }
            }
            ast::Expression::Binary { left, op, right, .. } => {
                let l = self.lower_expression(left)?;
                let r = self.lower_expression(right)?;
                Ok(IKunTree::Extension(op.clone(), vec![l, r]))
            }
            ast::Expression::Call { func, args, .. } => {
                let func_name = self.get_expression_name(func);
                if func_name == "printf" || func_name == "println" {
                    let mut arguments = vec![];
                    for arg in args {
                        arguments.push(self.lower_expression(arg)?);
                    }
                    Ok(IKunTree::CrossLangCall(
                        "native".to_string(),
                        "print".to_string(),
                        arguments,
                    ))
                } else {
                    let f = self.lower_expression(func)?;
                    let mut arguments = vec![];
                    for arg in args {
                        arguments.push(self.lower_expression(arg)?);
                    }
                    Ok(IKunTree::Apply(Box::new(f), arguments))
                }
            }
        }
    }

    fn get_expression_name(&self, expr: &ast::Expression) -> String {
        match expr {
            ast::Expression::Identifier { name, .. } => name.clone(),
            _ => "unknown".to_string(),
        }
    }

    fn get_loc(&self, node: &RedNode<GoLanguage>, source_id: u32) -> Loc {
        let span = node.span();
        Loc::new(source_id, span.start as u32, span.end as u32)
    }

    fn convert_red_to_uir(
        &self,
        builder: &mut IntentBuilder<()>,
        node: RedNode<GoLanguage>,
        source: &str,
        source_id: u32,
    ) -> Id {
        let kind = node.green.kind;
        let loc = self.get_loc(&node, source_id);

        match kind {
            GoSyntaxKind::SourceFile => {
                let mut items = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        if n.green.kind != GoSyntaxKind::Error {
                            let item = self.convert_red_to_uir(builder, n, source, source_id);
                            items.push(item);
                        }
                    }
                }
                builder.module("rusty-go", items)
            }
            _ => {
                // Placeholder for other elements
                builder.constant(0, loc)
            }
        }
    }
}
