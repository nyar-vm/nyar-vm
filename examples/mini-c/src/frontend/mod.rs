use oak_c::{CLexer, CParser, CRoot, ast::*, CLanguage, CElementType};
use chomsky_uast::UastNode;
use chomsky_source::Loc;
use oak_core::parser::{Parser, ParseSession};
use oak_core::lexer::{Lexer, LexerCache};
use oak_core::source::SourceText;
use oak_core::tree::{GreenTree, RedNode, RedTree};

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
        
        let green_node = parse_output.result.map_err(|e| format!("Parse error: {:?}", e))?;
        let red_node = RedNode::new(green_node, 0);
        
        Ok(self.convert_red_to_uast(red_node, source))
    }

    fn convert_red_to_uast(&self, node: RedNode<CLanguage>, source: &str) -> UastNode {
        match node.green.kind {
            CElementType::Root => {
                let mut items = Vec::new();
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        items.push(self.convert_red_to_uast(n, source));
                    }
                }
                UastNode::Module {
                    name: "mini-c".to_string(),
                    items,
                    loc: Loc::unknown(),
                }
            }
            CElementType::FunctionDefinition => {
                let mut name = "unknown".to_string();
                let mut body = vec![];
                
                for child in node.children() {
                    match child {
                        RedTree::Node(n) => {
                            if n.green.kind == CElementType::CompoundStatement {
                                if let UastNode::Block { body: b, .. } = self.convert_red_to_uast(n, source) {
                                    body = b;
                                }
                            }
                        }
                        RedTree::Leaf(l) => {
                            if let CElementType::Token(oak_c::lexer::CTokenType::Identifier) = l.kind.into() {
                                let span = l.span;
                                name = source[span.start..span.end].to_string();
                            }
                        }
                    }
                }
                
                UastNode::Function {
                    name,
                    params: vec![],
                    body,
                    loc: Loc::unknown(),
                }
            }
            CElementType::ExpressionStatement => {
                // Simplified: find the identifier if it's a call
                let mut callee = "unknown".to_string();
                for child in node.children() {
                    if let RedTree::Leaf(l) = child {
                        if let CElementType::Token(oak_c::lexer::CTokenType::Identifier) = l.kind.into() {
                            let span = l.span;
                            callee = source[span.start..span.end].to_string();
                        }
                    }
                }
                UastNode::Call {
                    callee: Box::new(UastNode::Literal(callee, Loc::unknown())),
                    args: vec![],
                    loc: Loc::unknown(),
                }
            }
            CElementType::CompoundStatement => {
                let mut items = Vec::new();
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        items.push(self.convert_red_to_uast(n, source));
                    }
                }
                UastNode::Block {
                    body: items,
                    loc: Loc::unknown(),
                }
            }
            CElementType::ReturnStatement => {
                UastNode::Return {
                    value: Some(Box::new(UastNode::Literal("return_val".to_string(), Loc::unknown()))),
                    loc: Loc::unknown(),
                }
            }
            _ => UastNode::Literal(format!("unsupported_{:?}", node.green.kind), Loc::unknown()),
        }
    }
}
