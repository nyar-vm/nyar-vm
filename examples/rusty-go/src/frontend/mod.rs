use chomsky_source::Loc;
use chomsky_uir::{EGraph, IKun, IKunTree, Id, IntentBuilder};
use oak_core::parser::ParseSession;
use oak_core::source::SourceText;
use oak_core::tree::{RedNode, RedTree};
use oak_core::{Lexer, LexerCache, Parser};
use oak_go::{ast, GoLanguage, GoLexer, GoParser, GoRoot, GoSyntaxKind};

#[derive(Default)]
pub struct MiniGoFrontend;

impl MiniGoFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, source: &str) -> Result<(EGraph<IKun, ()>, Id), String> {
        let language = GoLanguage::default();
        let lexer = GoLexer::new(language);
        let mut session = ParseSession::<GoLanguage>::new(16);

        let source_text = SourceText::new(source.to_string());

        let lex_output = lexer.lex(&source_text, &[], &mut session);
        let tokens = lex_output
            .result
            .map_err(|e| format!("Lex error: {:?}", e))?;
        session.set_lex_output(oak_core::LexOutput::<GoLanguage> {
            result: Ok(tokens),
            diagnostics: lex_output.diagnostics,
        });

        let parser = GoParser::new(language);
        let parse_output = Parser::<GoLanguage>::parse(&parser, &source_text, &[], &mut session);

        let green_node = parse_output
            .result
            .map_err(|e| format!("Parse error: {:?}", e))?;
        let red_node = RedNode::new(green_node, 0);

        let mut egraph = EGraph::new();
        let mut builder = IntentBuilder::new(&mut egraph);

        // Assume source_id 1 for the main file
        let root_id = self.convert_red_to_uir(&mut builder, red_node, source, 1);

        Ok((egraph, root_id))
    }

    pub fn lower(&self, ast: &GoRoot) -> Result<IKunTree, nyar_types::NyarError> {
        let mut items = vec![];

        for decl in &ast.declarations {
            match decl {
                ast::Declaration::Function(func) => {
                    let body = self.lower_block(&func.body)?;

                    // 为 main 函数创建导出
                    if func.name == "main" {
                        items.push(IKunTree::Export(
                            "main".to_string(),
                            Box::new(IKunTree::Lambda(vec![], Box::new(body))),
                        ));
                    } else {
                        items.push(IKunTree::Export(
                            func.name.clone(),
                            Box::new(IKunTree::Lambda(vec![], Box::new(body))),
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(IKunTree::Module("mini-go-program".to_string(), items))
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
            ast::Statement::Return { value, .. } => {
                let val = if let Some(v) = value {
                    self.lower_expression(v)?
                } else {
                    IKunTree::Seq(vec![])
                };
                Ok(IKunTree::Return(Box::new(val)))
            }
            _ => Ok(IKunTree::Seq(vec![])), // 简化处理其他语句
        }
    }

    fn lower_expression(&self, expr: &ast::Expression) -> Result<IKunTree, nyar_types::NyarError> {
        match expr {
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
                    Ok(IKunTree::Seq(vec![]))
                }
            }
            ast::Expression::Literal { value, .. } => {
                // 移除引号 if it's a string literal
                let s = if value.starts_with('"') && value.ends_with('"') {
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                Ok(IKunTree::StringConstant(s.to_string()))
            }
            _ => Ok(IKunTree::Seq(vec![])),
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
                builder.module("mini-go", items)
            }
            _ => {
                // Placeholder for other elements
                builder.constant(0, loc)
            }
        }
    }
}
