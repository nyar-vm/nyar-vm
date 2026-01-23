use oak_c::{CLexer, CParser, CLanguage, CElementType, CTokenType};
use chomsky_uir::{EGraph, Id, IntentBuilder, IKun};
use chomsky_source::Loc;
use oak_core::parser::{Parser, ParseSession};
use oak_core::source::SourceText;
use oak_core::tree::{RedNode, RedTree};
use oak_core::{Lexer, LexerCache};

pub struct MiniCFrontend;

impl MiniCFrontend {
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
        let loc = self.get_loc(&node, source_id);
        match node.green.kind {
            CElementType::Root => {
                let mut items = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        let item = self.convert_red_to_uir(builder, n, source, source_id);
                        items.push(item);
                    }
                }
                builder.module("mini-c", items)
            }
            CElementType::FunctionDefinition => {
                let mut name = "unknown".to_string();
                let mut body = vec![];
                let mut found_name = false;

                for child in node.children() {
                    match child {
                        RedTree::Node(n) => {
                            match n.green.kind {
                                CElementType::ReturnStatement
                                | CElementType::ExpressionStatement
                                | CElementType::DeclarationStatement
                                | CElementType::IfStatement
                                | CElementType::WhileStatement
                                | CElementType::ForStatement
                                | CElementType::CompoundStatement => {
                                    let stmt = self.convert_red_to_uir(builder, n, source, source_id);
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
                builder.function(&name, vec![], body)
            }
            CElementType::ReturnStatement => {
                let mut expr = None;
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        expr = Some(self.convert_red_to_uir(builder, n, source, source_id));
                        break;
                    }
                }
                if let Some(e) = expr {
                    builder.return_(e, loc)
                } else {
                    let zero = builder.constant(0, loc);
                    builder.return_(zero, loc)
                }
            }
            CElementType::DeclarationStatement => {
                let mut name = None;
                let mut value = None;
                let mut found_assign = false;

                for child in node.children() {
                    match child {
                        RedTree::Leaf(l) => {
                            let kind: CElementType = l.kind.into();
                            if let CElementType::Token(CTokenType::Identifier) = kind {
                                if name.is_none() {
                                    let s = l.span;
                                    name = Some(source[s.start..s.end].to_string());
                                }
                            } else if let CElementType::Token(CTokenType::Assign) = kind {
                                found_assign = true;
                            } else if let CElementType::Token(CTokenType::IntegerLiteral) = kind {
                                if found_assign {
                                    let s = l.span;
                                    if let Ok(val) = source[s.start..s.end].parse::<i64>() {
                                        value = Some(builder.constant(val, loc));
                                    }
                                }
                            }
                        }
                        RedTree::Node(n) => {
                            if found_assign {
                                value = Some(self.convert_red_to_uir(builder, n, source, source_id));
                            }
                        }
                    }
                }

                if let (Some(n), Some(v)) = (name, value) {
                    builder.assign(&n, v, loc)
                } else {
                    builder.constant(0, loc)
                }
            }
            CElementType::ExpressionStatement => {
                let children: Vec<_> = node.children().collect();
                
                // Handle binary operations: [left, op, right]
                if children.len() >= 3 {
                    // Try to find an operator in the middle
                    let mut op_idx = None;
                    for (i, child) in children.iter().enumerate() {
                        if let RedTree::Leaf(l) = child {
                            let kind: CElementType = l.kind.into();
                            match kind {
                                CElementType::Token(CTokenType::Plus) |
                                CElementType::Token(CTokenType::Minus) |
                                CElementType::Token(CTokenType::Star) |
                                CElementType::Token(CTokenType::Slash) |
                                CElementType::Token(CTokenType::Assign) |
                                CElementType::Token(CTokenType::Equal) |
                                CElementType::Token(CTokenType::NotEqual) |
                                CElementType::Token(CTokenType::Less) |
                                CElementType::Token(CTokenType::LessEqual) |
                                CElementType::Token(CTokenType::Greater) |
                                CElementType::Token(CTokenType::GreaterEqual) => {
                                    op_idx = Some(i);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }

                    if let Some(idx) = op_idx {
                        if idx > 0 && idx < children.len() - 1 {
                            if let (RedTree::Node(left), RedTree::Leaf(op_leaf), RedTree::Node(right)) = (&children[idx-1], &children[idx], &children[idx+1]) {
                                let left_id = self.convert_red_to_uir(builder, left.clone(), source, source_id);
                                let right_id = self.convert_red_to_uir(builder, right.clone(), source, source_id);
                                let op_span = op_leaf.span;
                                let op_text = &source[op_span.start..op_span.end];
                                
                                return match op_text {
                                    "=" => builder.assign_to_id(left_id, right_id, loc),
                                    _ => builder.binary_op(op_text, left_id, right_id, loc),
                                };
                            }
                        }
                    }
                }

                // Handle single child (literal or symbol or nested expression)
                if children.len() == 1 {
                    match &children[0] {
                        RedTree::Leaf(l) => {
                            let s = l.span;
                            let text = &source[s.start..s.end];
                            match l.kind.into() {
                                CElementType::Token(CTokenType::IntegerLiteral) => {
                                    if let Ok(val) = text.parse::<i64>() {
                                        return builder.constant(val, loc);
                                    }
                                }
                                CElementType::Token(CTokenType::FloatLiteral) => {
                                    if let Ok(val) = text.parse::<f64>() {
                                        return builder.float(val, loc);
                                    }
                                }
                                CElementType::Token(CTokenType::Identifier) => {
                                    return builder.symbol(text, loc);
                                }
                                _ => {}
                            }
                        }
                        RedTree::Node(n) => {
                            return self.convert_red_to_uir(builder, n.clone(), source, source_id);
                        }
                    }
                }

                builder.constant(0, loc)
            }
            CElementType::CompoundStatement => {
                let mut stmts = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        stmts.push(self.convert_red_to_uir(builder, n, source, source_id));
                    }
                }
                builder.block(stmts, loc)
            }
            CElementType::IfStatement => {
                let mut nodes = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        nodes.push(n);
                    }
                }
                if nodes.len() >= 2 {
                    let cond = self.convert_red_to_uir(builder, nodes[0].clone(), source, source_id);
                    let then_br = self.convert_red_to_uir(builder, nodes[1].clone(), source, source_id);
                    let else_br = if nodes.len() >= 3 {
                        self.convert_red_to_uir(builder, nodes[2].clone(), source, source_id)
                    } else {
                        builder.constant(0, loc)
                    };
                    builder.branch(cond, then_br, else_br, loc)
                } else {
                    builder.constant(0, loc)
                }
            }
            CElementType::WhileStatement => {
                let mut nodes = vec![];
                for child in node.children() {
                    if let RedTree::Node(n) = child {
                        nodes.push(n);
                    }
                }
                if nodes.len() >= 2 {
                    let cond = self.convert_red_to_uir(builder, nodes[0].clone(), source, source_id);
                    let body = self.convert_red_to_uir(builder, nodes[1].clone(), source, source_id);
                    builder.while_loop(cond, body, loc)
                } else {
                    builder.constant(0, loc)
                }
            }
            CElementType::Token(CTokenType::Identifier) => {
                let s = node.span();
                builder.symbol(&source[s.start..s.end], loc)
            }
            _ => {
                builder.constant(0, loc)
            }
        }
    }
}

