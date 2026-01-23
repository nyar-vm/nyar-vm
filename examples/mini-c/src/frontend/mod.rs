use oak_c::{CLexer, CParser, CLanguage, CElementType, CTokenType};
use chomsky_uir::{EGraph, Id, IntentBuilder};
use chomsky_source::Loc;
use oak_core::parser::{Parser, ParseSession};
use oak_core::lexer::{Lexer, LexerCache};
use oak_core::source::SourceText;
use oak_core::tree::{RedNode, RedTree};

pub struct MiniCFrontend;

impl MiniCFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, source: &str) -> Result<(EGraph, Id), String> {
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
        
        let green_node = parse_output.result.map_err(|e| format!("Parse error: {:?}", e))?;
        let red_node = RedNode::new(green_node, 0);
        
        let mut egraph = EGraph::default();
        let mut builder = IntentBuilder::new(&mut egraph);
        
        let root_id = self.convert_red_to_uir(&mut builder, red_node, source);
        
        Ok((egraph, root_id))
    }

    fn convert_red_to_uir(&self, builder: &mut IntentBuilder, node: RedNode<CLanguage>, source: &str) -> Id {
        match node.green.kind {
            CElementType::Root => {
                let mut items = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        let item = self.convert_red_to_uir(builder, n, source);
                        items.push(item);
                    }
                }
                // Wrap in a module for now, or just return a list if that's what we want.
                // But UIR usually expects a single root.
                // Let's make a module "mini-c".
                builder.module("mini-c", items)
            }
            CElementType::FunctionDefinition => {
                let mut name = "unknown".to_string();
                let mut body = vec![];
                let mut found_name = false;

                for child in node.children() {
                    match child {
                        RedTree::Node(n) => {
                            // Only process statements
                             match n.green.kind {
                                CElementType::ReturnStatement
                                | CElementType::ExpressionStatement
                                | CElementType::IfStatement
                                | CElementType::WhileStatement
                                | CElementType::ForStatement
                                | CElementType::CompoundStatement => {
                                    let stmt = self.convert_red_to_uir(builder, n, source);
                                    body.push(stmt);
                                }
                                _ => {}
                            }
                        }
                        RedTree::Leaf(l) => {
                            if let CElementType::Token(CTokenType::Identifier) = l.kind.into() {
                                if !found_name {
                                    let s = l.span;
                                    name = source[s.start..s.end].to_string();
                                    found_name = true;
                                }
                            }
                        }
                    }
                }
                // Empty params for now as in the original
                builder.function(&name, vec![], body)
            }
            CElementType::ReturnStatement => {
                let mut expr = None;
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        expr = Some(self.convert_red_to_uir(builder, n, source));
                        break;
                    }
                }
                if let Some(e) = expr {
                    builder.return_(e)
                } else {
                    // Return void/unit?
                    let unit = builder.constant(());
                    builder.return_(unit)
                }
            }
            CElementType::ExpressionStatement => {
                // If it's a leaf (literal/identifier), extract it
                for child in node.children() {
                    match child {
                        RedTree::Leaf(l) => {
                            let s = l.span;
                            let text = &source[s.start..s.end];
                            // Check if it's a number or identifier
                             if let CElementType::Token(CTokenType::IntegerLiteral) = l.kind.into() {
                                 if let Ok(val) = text.parse::<i64>() {
                                     return builder.constant(val);
                                 }
                             }
                             // Fallback to symbol for identifiers
                             return builder.symbol(text);
                        }
                        RedTree::Node(n) => {
                            return self.convert_red_to_uir(builder, n, source);
                        }
                    }
                }
                // Empty expression?
                builder.constant("empty_expr")
            }
            CElementType::CompoundStatement => {
                let mut stmts = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        stmts.push(self.convert_red_to_uir(builder, n, source));
                    }
                }
                builder.block(stmts)
            }
            CElementType::Token(t) => {
                // Just a literal for now
                let span = node.span();
                let text = &source[span.start..span.end];
                builder.constant(format!("token_{:?}:{}", t, text))
            }
            _ => {
                let span = node.span();
                let text = &source[span.start..span.end];
                // Return a string constant for unsupported nodes to avoid crashing
                builder.constant(format!("unsupported_{:?}: {}", node.green.kind, text.trim()))
            }
        }
    }
}
