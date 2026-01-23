use oak_c::{CLexer, CParser, CRoot, ast::*};
use chomsky_uast::UastNode;
use chomsky_source::Loc;

pub struct MiniCFrontend;

impl MiniCFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, source: &str) -> Result<UastNode, String> {
        let lexer = CLexer::new(source);
        let token_stream = lexer.tokenize();
        let mut parser = CParser::new(token_stream);
        
        let c_ast = parser.parse().map_err(|e| format!("Parse error: {:?}", e))?;
        Ok(self.convert_to_uast(&c_ast))
    }

    fn convert_to_uast(&self, root: &CRoot) -> UastNode {
        let mut items = Vec::new();
        for decl in &root.translation_unit.external_declarations {
            match decl {
                ExternalDeclaration::FunctionDefinition(f) => {
                    items.push(self.convert_function(f));
                }
                ExternalDeclaration::Declaration(d) => {
                    items.extend(self.convert_declaration(d));
                }
            }
        }

        UastNode::Module {
            name: "mini-c".to_string(),
            items,
            loc: Loc::unknown(),
        }
    }

    fn convert_function(&self, f: &FunctionDefinition) -> UastNode {
        let name = self.get_declarator_name(&f.declarator);
        let params = self.get_declarator_params(&f.declarator);
        let body = self.convert_compound_statement(&f.compound_statement);

        UastNode::Function {
            name,
            params,
            body,
            loc: Loc::unknown(),
        }
    }

    fn convert_declaration(&self, d: &Declaration) -> Vec<UastNode> {
        let mut nodes = Vec::new();
        for init in &d.init_declarators {
            let name = self.get_declarator_name(&init.declarator);
            if let Some(init_expr) = &init.initializer {
                let value = self.convert_initializer(init_expr);
                nodes.push(UastNode::Assign {
                    name,
                    value: Box::new(value),
                    loc: Loc::unknown(),
                });
            }
        }
        nodes
    }

    fn convert_compound_statement(&self, cs: &CompoundStatement) -> Vec<UastNode> {
        let mut nodes = Vec::new();
        for item in &cs.block_items {
            match item {
                BlockItem::Declaration(d) => nodes.extend(self.convert_declaration(d)),
                BlockItem::Statement(s) => nodes.push(self.convert_statement(s)),
            }
        }
        nodes
    }

    fn convert_statement(&self, s: &Statement) -> UastNode {
        match s {
            Statement::Expression(e) => {
                if let Some(expr) = e {
                    self.convert_expression(expr)
                } else {
                    UastNode::Tuple(vec![], Loc::unknown())
                }
            }
            Statement::Return(e) => {
                let arg = if let Some(expr) = e {
                    self.convert_expression(expr)
                } else {
                    UastNode::Tuple(vec![], Loc::unknown())
                };
                UastNode::Call {
                    callee: "return".to_string(),
                    args: vec![arg],
                    loc: Loc::unknown(),
                }
            }
            _ => UastNode::Literal("unsupported_stmt".to_string(), Loc::unknown()),
        }
    }

    fn convert_expression(&self, e: &Expression) -> UastNode {
        match e {
            Expression::Primary(p) => self.convert_primary(p),
            Expression::Binary(left, op, right) => {
                UastNode::Call {
                    callee: format!("{:?}", op),
                    args: vec![self.convert_expression(left), self.convert_expression(right)],
                    loc: Loc::unknown(),
                }
            }
            _ => UastNode::Literal("unsupported_expr".to_string(), Loc::unknown()),
        }
    }

    fn convert_primary(&self, p: &PrimaryExpression) -> UastNode {
        match p {
            PrimaryExpression::Identifier(s, _) => UastNode::Literal(s.clone(), Loc::unknown()),
            PrimaryExpression::Constant(c, _) => {
                match c {
                    Constant::Integer(v, _) => UastNode::Literal(v.to_string(), Loc::unknown()),
                    Constant::Float(v, _) => UastNode::Literal(v.clone(), Loc::unknown()),
                    _ => UastNode::Literal("0".to_string(), Loc::unknown()),
                }
            }
            PrimaryExpression::StringLiteral(s, _) => UastNode::Literal(format!("\"{}\"", s), Loc::unknown()),
            _ => UastNode::Literal("unsupported_primary".to_string(), Loc::unknown()),
        }
    }

    fn convert_initializer(&self, i: &Initializer) -> UastNode {
        match i {
            Initializer::Expression(e) => self.convert_expression(e),
            _ => UastNode::Literal("unsupported_init".to_string(), Loc::unknown()),
        }
    }

    fn get_declarator_name(&self, d: &Declarator) -> String {
        self.get_direct_declarator_name(&d.direct_declarator)
    }

    fn get_direct_declarator_name(&self, d: &DirectDeclarator) -> String {
        match d {
            DirectDeclarator::Identifier(name, _) => name.clone(),
            DirectDeclarator::Declarator(inner) => self.get_declarator_name(inner),
            DirectDeclarator::Function { direct, .. } => self.get_direct_declarator_name(direct),
            _ => "unknown".to_string(),
        }
    }

    fn get_declarator_params(&self, d: &Declarator) -> Vec<UastNode> {
        match &d.direct_declarator {
            DirectDeclarator::Function { parameters, .. } => {
                if let Some(params) = parameters {
                    params.iter().map(|p| {
                        if let Some(decl) = &p.declarator {
                            UastNode::Literal(self.get_declarator_name(decl), Loc::unknown())
                        } else {
                            UastNode::Literal("param".to_string(), Loc::unknown())
                        }
                    }).collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}
