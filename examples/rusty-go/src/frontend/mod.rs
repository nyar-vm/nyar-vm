use oak_c::{CLexer, CParser, CLanguage, CElementType};
use chomsky_uir::{EGraph, Id, IntentBuilder, IKun, IKunTree};
use chomsky_source::Loc;
use oak_core::parser::ParseSession;
use oak_core::source::SourceText;
use oak_core::tree::{RedNode, RedTree};
use oak_core::{Lexer, Parser, LexerCache};

#[derive(Default)]
pub struct MiniGoFrontend;

impl MiniGoFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, source: &str) -> Result<(EGraph<IKun, ()>, Id), String> {
        let language = CLanguage::default();
        let lexer = CLexer::new(language);
        let mut session = ParseSession::<CLanguage>::new(16);
        
        let source_text = SourceText::new(source.to_string());
        
        let lex_output = lexer.lex(&source_text, &[], &mut session);
        let tokens = lex_output.result.map_err(|e| format!("Lex error: {:?}", e))?;
        session.set_lex_output(oak_core::LexOutput::<CLanguage> {
            result: Ok(tokens),
            diagnostics: lex_output.diagnostics,
        });
        
        let parser = CParser::new(language);
        let parse_output = Parser::<CLanguage>::parse(&parser, &source_text, &[], &mut session);
        
        let green_node = parse_output.result.map_err(|e| format!("Parse error: {:?}", e))?;
        let red_node = RedNode::new(green_node, 0);
        
        let mut egraph = EGraph::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        
        // Assume source_id 1 for the main file
        let root_id = self.convert_red_to_uir(&mut builder, red_node, source, 1);
        
        Ok((egraph, root_id))
    }

    pub fn lower(&self, ast: &oak_c::CRoot) -> Result<IKunTree, nyar_types::NyarError> {
        let mut items = vec![];

        for decl in &ast.translation_unit.external_declarations {
            match decl {
                oak_c::ast::ExternalDeclaration::FunctionDefinition(func) => {
                    let name = self.get_declarator_name(&func.declarator);
                    let body = self.lower_compound_statement(&func.compound_statement)?;
                    
                    // 为 main 函数创建导出
                    if name == "main" {
                        items.push(IKunTree::Export("main".to_string(), Box::new(IKunTree::Lambda(vec![], Box::new(body)))));
                    } else {
                        // 其他函数也可以作为普通导出，因为 IKunTree 没有专门的 Function 变体
                        items.push(IKunTree::Export(name, Box::new(IKunTree::Lambda(vec![], Box::new(body)))));
                    }
                }
                _ => {}
            }
        }

        Ok(IKunTree::Module("mini-go-program".to_string(), items))
    }

    fn get_declarator_name(&self, declarator: &oak_c::ast::Declarator) -> String {
        self.get_direct_declarator_name(&declarator.direct_declarator)
    }

    fn get_direct_declarator_name(&self, direct: &oak_c::ast::DirectDeclarator) -> String {
        match direct {
            oak_c::ast::DirectDeclarator::Identifier(name, _) => name.clone(),
            oak_c::ast::DirectDeclarator::Function { declarator, .. } => {
                self.get_direct_declarator_name(declarator)
            }
            oak_c::ast::DirectDeclarator::Declarator(decl) => {
                self.get_declarator_name(decl)
            }
            _ => "unknown".to_string(),
        }
    }

    fn lower_compound_statement(&self, stmt: &oak_c::ast::CompoundStatement) -> Result<IKunTree, nyar_types::NyarError> {
        let mut items = vec![];
        for item in &stmt.block_items {
            match item {
                oak_c::ast::BlockItem::Statement(s) => {
                    items.push(self.lower_statement(s)?);
                }
                _ => {}
            }
        }
        Ok(IKunTree::Seq(items))
    }

    fn lower_statement(&self, stmt: &oak_c::ast::Statement) -> Result<IKunTree, nyar_types::NyarError> {
        match stmt {
            oak_c::ast::Statement::Expression(expr_stmt) => {
                if let Some(expr) = &expr_stmt.expression {
                    self.lower_expression(expr)
                } else {
                    Ok(IKunTree::Seq(vec![]))
                }
            }
            oak_c::ast::Statement::Compound(comp) => self.lower_compound_statement(comp),
            _ => Ok(IKunTree::Seq(vec![])), // 简化处理其他语句
        }
    }

    fn lower_expression(&self, expr: &oak_c::ast::Expression) -> Result<IKunTree, nyar_types::NyarError> {
        match &*expr.kind {
            oak_c::ast::ExpressionKind::FunctionCall { function, arguments, .. } => {
                let func_name = self.get_expression_name(function);
                if func_name == "printf" || func_name == "println" {
                    let mut args = vec![];
                    for arg in arguments {
                        args.push(self.lower_expression(arg)?);
                    }
                    Ok(IKunTree::CrossLangCall("native".to_string(), "System.Console.WriteLine".to_string(), args))
                } else {
                    Ok(IKunTree::Seq(vec![]))
                }
            }
            oak_c::ast::ExpressionKind::StringLiteral(s, _) => {
                Ok(IKunTree::StringConstant(s.clone()))
            }
            _ => Ok(IKunTree::Seq(vec![])),
        }
    }

    fn get_expression_name(&self, expr: &oak_c::ast::Expression) -> String {
        match &*expr.kind {
            oak_c::ast::ExpressionKind::Identifier(name, _) => name.clone(),
            _ => "unknown".to_string(),
        }
    }

    fn get_loc(&self, node: &RedNode<CLanguage>, source_id: u32) -> Loc {
        let span = node.span();
        Loc::new(source_id, span.start as u32, span.end as u32)
    }

    fn convert_red_to_uir(&self, builder: &mut IntentBuilder<()>, node: RedNode<CLanguage>, source: &str, source_id: u32) -> Id {
        let kind = node.green.kind;
        let loc = self.get_loc(&node, source_id);
        
        match kind {
            CElementType::Root => {
                let mut items = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        if n.green.kind != CElementType::Error {
                            let item = self.convert_red_to_uir(builder, n, source, source_id);
                            items.push(item);
                        }
                    }
                }
                builder.module("mini-go", items)
            }
            _ => {
                // Placeholder for other elements
                builder.constant(0, loc)
            }
        }
    }
}
