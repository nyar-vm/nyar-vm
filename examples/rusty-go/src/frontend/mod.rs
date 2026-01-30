use oak_c::{CLexer, CParser, CLanguage, CElementType};
use chomsky_uir::{EGraph, Id, IntentBuilder, IKun};
use chomsky_source::Loc;
use oak_core::parser::ParseSession;
use oak_core::source::SourceText;
use oak_core::tree::{RedNode, RedTree};
use oak_core::Lexer;

pub struct MiniGoFrontend;

impl MiniGoFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, source: &str) -> Result<(EGraph<IKun, ()>, Id), String> {
        let language = CLanguage::default();
        let lexer = CLexer::new(&language);
        let mut session = ParseSession::<CLanguage>::new(16);
        
        let source_text = SourceText::new(source.to_string());
        
        let lex_output = lexer.lex(&source_text, &[], &mut session);
        let tokens = lex_output.result.map_err(|e| format!("Lex error: {:?}", e))?;
        session.set_lex_output(oak_core::LexOutput::<CLanguage> {
            result: Ok(tokens),
            diagnostics: lex_output.diagnostics,
        });
        
        let parser = CParser::new(&language);
        let parse_output = parser.parse(&source_text, &[], &mut session);
        
        let green_node = parse_output.result.map_err(|e| format!("Parse error: {:?}", e))?;
        let red_node = RedNode::new(green_node, 0);
        
        let mut egraph = EGraph::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        
        // Assume source_id 1 for the main file
        let root_id = self.convert_red_to_uir(&mut builder, red_node, source, 1);
        
        Ok((egraph, root_id))
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
