use nyar_types::{NyarContext, NyarError, NyarFrontend, Id, Vfs, Loc};
use oak_cobol::{CobolLanguage, CobolBuilder, ast::CobolRoot};
use oak_core::{Builder, SourceText};

pub struct RustyCobolFrontend {
    language: CobolLanguage,
}

impl RustyCobolFrontend {
    pub fn new() -> Self {
        Self {
            language: CobolLanguage::default(),
        }
    }
}

impl Default for RustyCobolFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyCobolFrontend {
    type Language = CobolLanguage;

    fn parse(&self, source: &str) -> Result<CobolRoot<'static>, NyarError> {
        let builder = CobolBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<CobolLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &CobolRoot<'static>, ctx: &mut NyarContext<V>) -> Id {
        let mut lowerer = CobolLowerer::new(ctx);
        lowerer.lower_root(ast)
    }
}

struct CobolLowerer<'a, 'b, V: Vfs, A: chomsky_uir::Analysis<chomsky_uir::IKun>> {
    ctx: &'a mut NyarContext<'b, V, A>,
}

impl<'a, 'b, V: Vfs, A: chomsky_uir::Analysis<chomsky_uir::IKun>> CobolLowerer<'a, 'b, V, A> {
    fn new(ctx: &'a mut NyarContext<'b, V, A>) -> Self {
        Self { ctx }
    }

    fn lower_root(&mut self, root: &CobolRoot<'static>) -> Id {
        let mut items = Vec::new();
        // Traverse the red tree and generate IKun
        self.lower_node(root.syntax, &mut items);
        
        self.ctx.builder.module("rusty-cobol-program", items, Loc::default())
    }

    fn lower_node(&mut self, node: oak_cobol::ast::CobolNode<'static>, items: &mut Vec<Id>) {
        use oak_cobol::parser::CobolElementType;
        use oak_core::RedTree;

        for child in node.children() {
            if let RedTree::Node(n) = child {
                match n.green.kind {
                    CobolElementType::ProcedureDivision => {
                        self.lower_procedure_division(n, items);
                    }
                    _ => {
                        // Recurse for other divisions for now
                        self.lower_node(n, items);
                    }
                }
            }
        }
    }

    fn lower_procedure_division(&mut self, node: oak_cobol::ast::CobolNode<'static>, items: &mut Vec<Id>) {
        use oak_cobol::parser::CobolElementType;
        use oak_core::RedTree;

        let mut stmts = Vec::new();
        for child in node.children() {
            if let RedTree::Node(n) = child {
                match n.green.kind {
                    CobolElementType::DisplayStatement => {
                        stmts.push(self.lower_display_statement(n));
                    }
                    CobolElementType::StopStatement => {
                        stmts.push(self.lower_stop_statement(n));
                    }
                    _ => {}
                }
            }
        }
        
        let main_func = self.ctx.builder.function("main", Vec::new(), stmts, Loc::default());
        items.push(main_func);
    }

    fn lower_display_statement(&mut self, node: oak_cobol::ast::CobolNode<'static>) -> Id {
        // Simplified: just collect all literals/identifiers and call a "display" extension
        let mut args = Vec::new();
        for child in node.children() {
            match child {
                oak_core::RedTree::Leaf(t) => {
                    match t.kind {
                        oak_cobol::lexer::CobolTokenType::StringLiteral => {
                            // Extract text and create IKun string
                            // This requires access to source text which we don't have easily here
                            // For now use a placeholder
                            args.push(self.ctx.builder.string("TODO: string", Loc::default()));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        self.ctx.builder.extension("display", args, Loc::default())
    }

    fn lower_stop_statement(&mut self, _node: oak_cobol::ast::CobolNode<'static>) -> Id {
        self.ctx.builder.extension("stop", Vec::new(), Loc::default())
    }
}
