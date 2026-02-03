use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_go::{ast, GoBuilder, GoLanguage, GoRoot};
use oak_core::source::SourceText;
use oak_core::parser::session::ParseSession;
use chomsky_uir::Id;
use chomsky_source::Loc;
use std::ops::Range;

/// Rusty Go 前端实现
#[derive(Default)]
pub struct RustyGoFrontend {
    language: GoLanguage,
}

impl RustyGoFrontend {
    pub fn new() -> Self {
        Self {
            language: GoLanguage::default(),
        }
    }
}

impl NyarFrontend for RustyGoFrontend {
    type Language = GoLanguage;

    fn parse(&self, source: &str) -> Result<GoRoot, NyarError> {
        use oak_core::Builder;
        let builder = GoBuilder::new(&self.language);
        let source_text = SourceText::new(source.to_string());
        let mut session = ParseSession::<GoLanguage>::default();
        let output = builder.build(&source_text, &[], &mut session);

        output.result.map_err(|e| NyarError::Compile(format!("Build error: {:?}", e)))
    }

    fn lower_unified(&self, ast: &GoRoot, ctx: &mut NyarContext) -> Id {
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

    fn to_loc(&self, range: Range<usize>) -> Loc {
        self.ctx.loc(range.start as u32, range.end as u32)
    }

    fn convert_root(&mut self, root: &GoRoot) -> Id {
        let mut items = Vec::new();
        for decl in &root.declarations {
            items.push(self.convert_declaration(decl));
        }
        self.ctx.builder().module("main", items)
    }

    fn convert_declaration(&mut self, decl: &ast::Declaration) -> Id {
        match decl {
            ast::Declaration::Function(func) => self.convert_function(func),
            ast::Declaration::Variable(var) => self.convert_variable(var),
            ast::Declaration::Const(c) => self.convert_const(c),
            _ => self.ctx.builder().constant(0, Loc::default()),
        }
    }

    fn convert_function(&mut self, func: &ast::Function) -> Id {
        let loc = self.to_loc(func.span.clone().into());
        let mut params = Vec::new();
        self.ctx.scopes.push_scope();
        for p in &func.params {
            params.push(self.ctx.scopes.declare_variable(&p.name));
        }
        let body = self.convert_block(&func.body);
        self.ctx.scopes.pop_scope();

        let lambda = self.ctx.builder().function(&func.name, params, vec![body]);
        self.ctx.builder().assign(&func.name, lambda, loc)
    }

    fn convert_variable(&mut self, var: &ast::Variable) -> Id {
        let loc = self.to_loc(var.span.clone().into());
        let name = self.ctx.scopes.declare_variable(&var.name);
        let val = if let Some(v) = &var.value {
            self.convert_expression(v)
        } else {
            self.ctx.builder().constant(0, loc.clone())
        };
        self.ctx.builder().assign(&name, val, loc)
    }

    fn convert_const(&mut self, c: &ast::Const) -> Id {
        let loc = self.to_loc(c.span.clone().into());
        let name = self.ctx.scopes.declare_variable(&c.name);
        let val = self.convert_expression(&c.value);
        self.ctx.builder().assign(&name, val, loc)
    }

    fn convert_block(&mut self, block: &ast::Block) -> Id {
        let loc = self.to_loc(block.span.clone().into());
        let mut ids = Vec::new();
        self.ctx.scopes.push_scope();
        for stmt in &block.statements {
            ids.push(self.convert_statement(stmt));
        }
        self.ctx.scopes.pop_scope();
        self.ctx.builder().block(ids, loc)
    }

    fn convert_statement(&mut self, stmt: &ast::Statement) -> Id {
        let span: Range<usize> = match stmt {
            ast::Statement::Expression(expr) => self.get_expr_span(expr),
            ast::Statement::Assignment { span, .. } => span.clone().into(),
            ast::Statement::Return { span, .. } => span.clone().into(),
            ast::Statement::If { span, .. } => span.clone().into(),
            ast::Statement::For { span, .. } => span.clone().into(),
        };
        let loc = self.to_loc(span);
        match stmt {
            ast::Statement::Expression(expr) => self.convert_expression(expr),
            ast::Statement::Assignment { target, value, .. } => {
                let val = self.convert_expression(value);
                let name = self.ctx.scopes.resolve_variable(target);
                let target_id = self.ctx.builder().symbol(&name, loc.clone());
                self.ctx.builder().assign_to_id(target_id, val, loc)
            }
            ast::Statement::Return { value, .. } => {
                let val = if let Some(v) = value {
                    self.convert_expression(v)
                } else {
                    self.ctx.builder().constant(0, loc.clone())
                };
                self.ctx.builder().return_(val, loc)
            }
            ast::Statement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                let cond = self.convert_expression(condition);
                let then_id = self.convert_block(then_block);
                let else_id = if let Some(eb) = else_block {
                    self.convert_block(eb)
                } else {
                    self.ctx.builder().constant(0, loc.clone())
                };
                self.ctx.builder().branch(cond, then_id, else_id, loc)
            }
            ast::Statement::For {
                init,
                condition,
                post,
                body,
                ..
            } => {
                self.ctx.scopes.push_scope();
                let mut init_id = self.ctx.builder().constant(0, loc.clone());
                if let Some(i) = init {
                    init_id = self.convert_statement(i);
                }
                let cond = if let Some(c) = condition {
                    self.convert_expression(c)
                } else {
                    self.ctx.builder().constant(1, loc.clone())
                };
                let mut post_id = self.ctx.builder().constant(0, loc.clone());
                if let Some(p) = post {
                    post_id = self.convert_statement(p);
                }
                let body_id = self.convert_block(body);
                self.ctx.scopes.pop_scope();
                self.ctx.builder().extension("for", vec![init_id, cond, post_id, body_id], loc)
            }
        }
    }

    fn get_expr_span(&self, expr: &ast::Expression) -> Range<usize> {
        match expr {
            ast::Expression::Identifier { span, .. } => span.clone().into(),
            ast::Expression::Literal { span, .. } => span.clone().into(),
            ast::Expression::Binary { span, .. } => span.clone().into(),
            ast::Expression::Call { span, .. } => span.clone().into(),
        }
    }

    fn convert_expression(&mut self, expr: &ast::Expression) -> Id {
        let span = self.get_expr_span(expr);
        let loc = self.to_loc(span);
        match expr {
            ast::Expression::Identifier { name, .. } => {
                let resolved = self.ctx.scopes.resolve_variable(name);
                self.ctx.builder().symbol(&resolved, loc)
            }
            ast::Expression::Literal { value, .. } => {
                if value.starts_with('"') && value.ends_with('"') {
                    self.ctx.builder().extension("string", vec![], loc) // Simplified string
                } else if value == "true" {
                    self.ctx.builder().constant(1, loc)
                } else if value == "false" {
                    self.ctx.builder().constant(0, loc)
                } else if let Ok(n) = value.parse::<i64>() {
                    self.ctx.builder().constant(n, loc)
                } else {
                    self.ctx.builder().constant(0, loc)
                }
            }
            ast::Expression::Binary { left, op, right, .. } => {
                let l = self.convert_expression(left);
                let r = self.convert_expression(right);
                self.ctx.builder().binary_op(op, l, r, loc)
            }
            ast::Expression::Call { func, args, .. } => {
                let mut arguments = Vec::new();
                for arg in args {
                    arguments.push(self.convert_expression(arg));
                }

                if let ast::Expression::Identifier { name, .. } = &**func {
                    if let Some(intrinsic) = self.ctx.map_intrinsic(name, arguments.clone(), loc.clone()) {
                        return intrinsic;
                    }
                }

                let f = self.convert_expression(func);
                self.ctx.builder().call(f, arguments, loc)
            }
        }
    }
}
