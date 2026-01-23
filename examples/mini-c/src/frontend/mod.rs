use oak_c::{CLexer, CParser, CRoot, ast::*, CLanguage};
use chomsky_uast::UastNode;
use chomsky_source::Loc;
use oak_core::parser::{Parser, ParseSession};
use oak_core::lexer::{Lexer, LexerCache};
use oak_core::source::SourceText;

pub struct MiniCFrontend;

impl MiniCFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, source: &str) -> Result<UastNode, String> {
        let language = CLanguage::default();
        let lexer = CLexer::new(&language);
        let mut session = ParseSession::<CLanguage>::new(16);
        
        let source_text = SourceText::new(source.to_string());
        
        let lex_output = lexer.lex(&source_text, &[], &mut session);
        let tokens = lex_output.result.map_err(|e| format!("Lex error: {:?}", e))?;
        session.set_lex_output(oak_core::errors::OakDiagnostics {
            result: Ok(tokens),
            diagnostics: lex_output.diagnostics,
        });
        
        let parser = CParser::new(&language);
        let parse_output = parser.parse(&source_text, &[], &mut session);
        
        // Actually, CParser in oak-c seems to return a GreenNode.
        // We need a way to get CRoot. Since I don't see a clear way in oak-c yet,
        // and rusty-c seems to have a different setup, I'll temporarily 
        // mock the conversion or find the right way to cast.
        // For now, let's assume we can't easily get CRoot from GreenNode without more info.
        // I'll try to use a placeholder or see if I can find the cast logic.
        
        Err("Conversion from GreenNode to CRoot not implemented yet".to_string())
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
                if let Some(expr) = &e.expression {
                    self.convert_expression(expr)
                } else {
                    UastNode::Tuple(vec![], Loc::unknown())
                }
            }
            Statement::Jump(j) => {
                match j {
                    JumpStatement::Return(e, _) => {
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
                    _ => UastNode::Literal("unsupported_jump".to_string(), Loc::unknown()),
                }
            }
            _ => UastNode::Literal("unsupported_stmt".to_string(), Loc::unknown()),
        }
    }

    fn convert_expression(&self, e: &Expression) -> UastNode {
        match e.kind.as_ref() {
            ExpressionKind::Identifier(s, _) => UastNode::Literal(s.clone(), Loc::unknown()),
            ExpressionKind::Constant(c, _) => {
                match c {
                    Constant::Integer(v, _) => UastNode::Literal(v.to_string(), Loc::unknown()),
                    Constant::Float(v, _) => UastNode::Literal(v.to_string(), Loc::unknown()),
                    Constant::Character(v, _) => UastNode::Literal(v.to_string(), Loc::unknown()),
                }
            }
            ExpressionKind::StringLiteral(s, _) => UastNode::Literal(format!("\"{}\"", s), Loc::unknown()),
            ExpressionKind::Binary { left, operator, right, .. } => {
                UastNode::Call {
                    callee: format!("{:?}", operator),
                    args: vec![self.convert_expression(left), self.convert_expression(right)],
                    loc: Loc::unknown(),
                }
            }
            _ => UastNode::Literal("unsupported_expr".to_string(), Loc::unknown()),
        }
    }

    fn convert_initializer(&self, i: &Initializer) -> UastNode {
        match i {
            Initializer::AssignmentExpression(e) => self.convert_expression(e),
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
            DirectDeclarator::Function { declarator, .. } => self.get_direct_declarator_name(declarator),
            _ => "unknown".to_string(),
        }
    }

    fn get_declarator_params(&self, d: &Declarator) -> Vec<UastNode> {
        match &d.direct_declarator {
            DirectDeclarator::Function { parameter_type_list, .. } => {
                if let Some(params) = parameter_type_list {
                    params.parameter_list.iter().map(|p| {
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
