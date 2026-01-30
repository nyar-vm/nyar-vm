use chomsky_source::Loc;
use chomsky_uir::{EGraph, IKun, Id, IntentBuilder};
use oak_c::{CElementType, CLanguage, CLexer, CParser, CTokenType};
use oak_core::parser::{ParseSession, Parser};
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
        let lexer = CLexer::new(language);
        let mut session = ParseSession::<CLanguage>::new(16);

        let source_text = SourceText::new(source.to_string());

        let lex_output = lexer.lex(&source_text, &[], &mut session);
        let tokens = lex_output
            .result
            .map_err(|e| format!("Lex error: {:?}", e))?;
        session.set_lex_output(oak_core::LexOutput::<CLanguage> {
            result: Ok(tokens),
            diagnostics: lex_output.diagnostics,
        });

        let parser = CParser::new(language);
        let parse_output = parser.parse(&source_text, &[], &mut session);

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

    fn get_loc(&self, node: &RedNode<CLanguage>, source_id: u32) -> Loc {
        let span = node.span();
        Loc::new(source_id, span.start as u32, span.end as u32)
    }

    fn get_text<'a>(&self, span: core::range::Range<usize>, source: &'a str) -> &'a str {
        let start = span.start.min(source.len());
        let end = span.end.min(source.len());
        &source[start..end]
    }

    fn convert_red_to_uir(
        &self,
        builder: &mut IntentBuilder<()>,
        node: RedNode<CLanguage>,
        source: &str,
        source_id: u32,
    ) -> Id {
        self.convert_tree_to_uir(builder, RedTree::Node(node), source, source_id)
    }

    fn convert_tree_to_uir(
        &self,
        builder: &mut IntentBuilder<()>,
        tree: RedTree<CLanguage>,
        source: &str,
        source_id: u32,
    ) -> Id {
        match tree {
            RedTree::Node(node) => {
                let kind = node.green.kind;
                let loc = self.get_loc(&node, source_id);
                println!("DEBUG: Node kind: {:?}", kind);
                for (i, child) in node.children().enumerate() {
                    match child {
                        RedTree::Node(n) => {
                            println!("  DEBUG: Child {} Node: {:?}", i, n.green.kind)
                        }
                        RedTree::Leaf(l) => {
                            let text = self.get_text(l.span, source);
                            println!("  DEBUG: Child {} Leaf: {:?} '{}'", i, l.kind, text);
                        }
                    }
                }
                match kind {
                    CElementType::Root => {
                        let mut items = vec![];
                        for child in node.children() {
                            if let RedTree::Node(n) = child {
                                if n.green.kind != CElementType::Error {
                                    let item =
                                        self.convert_red_to_uir(builder, n, source, source_id);
                                    items.push(item);
                                }
                            }
                        }
                        builder.module("mini-c", items)
                    }
                    CElementType::FunctionDefinition => {
                        let mut name = "unknown".to_string();
                        let mut body = vec![];
                        let mut params = vec![];
                        let mut found_name = false;

                        for child in node.children() {
                            match child {
                                RedTree::Node(n) => match n.green.kind {
                                    CElementType::ParameterList => {
                                        for p_child in n.children() {
                                            if let RedTree::Leaf(pl) = p_child {
                                                let kind: CElementType = pl.kind.into();
                                                if let CElementType::Token(CTokenType::Identifier) =
                                                    kind
                                                {
                                                    params.push(
                                                        self.get_text(pl.span, source).to_string(),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    CElementType::Error => {}
                                    _ => {
                                        let stmt =
                                            self.convert_red_to_uir(builder, n, source, source_id);
                                        body.push(stmt);
                                    }
                                },
                                RedTree::Leaf(l) => {
                                    let kind: CElementType = l.kind.into();
                                    match kind {
                                        CElementType::Token(CTokenType::Identifier) => {
                                            if !found_name {
                                                name = self.get_text(l.span, source).to_string();
                                                found_name = true;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        builder.function(&name, params, body)
                    }
                    CElementType::ReturnStatement => {
                        let filtered_children: Vec<_> = node
                            .children()
                            .filter(|child| match child {
                                RedTree::Leaf(l) => {
                                    let kind: CElementType = l.kind.into();
                                    !matches!(
                                        kind,
                                        CElementType::Token(CTokenType::Whitespace)
                                            | CElementType::Token(CTokenType::Comment)
                                    )
                                }
                                _ => true,
                            })
                            .collect();

                        let mut expr = None;
                        for child in filtered_children {
                            match child {
                                RedTree::Node(n) => {
                                    if n.green.kind == CElementType::ExpressionStatement {
                                        // Look inside ExpressionStatement
                                        for subchild in n.children() {
                                            if let RedTree::Leaf(l) = subchild {
                                                let kind: CElementType = l.kind.into();
                                                if matches!(
                                                    kind,
                                                    CElementType::Token(
                                                        CTokenType::IntegerLiteral
                                                            | CTokenType::FloatLiteral
                                                            | CTokenType::Identifier
                                                    )
                                                ) {
                                                    expr = Some(self.convert_tree_to_uir(
                                                        builder,
                                                        RedTree::Leaf(l),
                                                        source,
                                                        source_id,
                                                    ));
                                                    break;
                                                }
                                            } else if let RedTree::Node(sn) = subchild {
                                                expr = Some(self.convert_red_to_uir(
                                                    builder, sn, source, source_id,
                                                ));
                                                break;
                                            }
                                        }
                                    } else {
                                        expr = Some(
                                            self.convert_red_to_uir(builder, n, source, source_id),
                                        );
                                    }
                                    if expr.is_some() {
                                        break;
                                    }
                                }
                                RedTree::Leaf(l) => {
                                    let kind: CElementType = l.kind.into();
                                    if matches!(
                                        kind,
                                        CElementType::Token(
                                            CTokenType::IntegerLiteral
                                                | CTokenType::FloatLiteral
                                                | CTokenType::Identifier
                                        )
                                    ) {
                                        expr = Some(self.convert_tree_to_uir(
                                            builder,
                                            RedTree::Leaf(l),
                                            source,
                                            source_id,
                                        ));
                                        break;
                                    }
                                }
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
                        let filtered_children: Vec<_> = node
                            .children()
                            .filter(|child| match child {
                                RedTree::Leaf(l) => {
                                    let kind: CElementType = l.kind.into();
                                    !matches!(
                                        kind,
                                        CElementType::Token(CTokenType::Whitespace)
                                            | CElementType::Token(CTokenType::Comment)
                                    )
                                }
                                _ => true,
                            })
                            .collect();

                        let mut name = None;
                        let mut value = None;
                        let mut found_assign = false;

                        for child in filtered_children {
                            match child {
                                RedTree::Leaf(l) => {
                                    let kind: CElementType = l.kind.into();
                                    if let CElementType::Token(CTokenType::Identifier) = kind {
                                        if name.is_none() {
                                            name = Some(self.get_text(l.span, source).to_string());
                                        }
                                    } else if let CElementType::Token(CTokenType::Assign) = kind {
                                        found_assign = true;
                                    } else if let CElementType::Token(
                                        CTokenType::IntegerLiteral | CTokenType::FloatLiteral,
                                    ) = kind
                                    {
                                        if found_assign {
                                            value = Some(self.convert_tree_to_uir(
                                                builder,
                                                RedTree::Leaf(l),
                                                source,
                                                source_id,
                                            ));
                                        }
                                    }
                                }
                                RedTree::Node(n) => {
                                    if found_assign {
                                        if n.green.kind == CElementType::ExpressionStatement {
                                            for subchild in n.children() {
                                                if let RedTree::Leaf(l) = subchild {
                                                    let kind: CElementType = l.kind.into();
                                                    if matches!(
                                                        kind,
                                                        CElementType::Token(
                                                            CTokenType::IntegerLiteral
                                                                | CTokenType::FloatLiteral
                                                                | CTokenType::Identifier
                                                        )
                                                    ) {
                                                        value = Some(self.convert_tree_to_uir(
                                                            builder,
                                                            RedTree::Leaf(l),
                                                            source,
                                                            source_id,
                                                        ));
                                                        break;
                                                    }
                                                } else if let RedTree::Node(sn) = subchild {
                                                    value = Some(self.convert_red_to_uir(
                                                        builder, sn, source, source_id,
                                                    ));
                                                    break;
                                                }
                                            }
                                        } else {
                                            value =
                                                Some(self.convert_red_to_uir(
                                                    builder, n, source, source_id,
                                                ));
                                        }
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
                        let filtered_children: Vec<_> = node
                            .children()
                            .filter(|child| match child {
                                RedTree::Leaf(l) => {
                                    let kind: CElementType = l.kind.into();
                                    !matches!(
                                        kind,
                                        CElementType::Token(CTokenType::Whitespace)
                                            | CElementType::Token(CTokenType::Comment)
                                    )
                                }
                                _ => true,
                            })
                            .collect();

                        // Handle function calls (Identifier followed by LeftParen)
                        if filtered_children.len() >= 3 {
                            let mut is_call = false;
                            if let RedTree::Leaf(l) = &filtered_children[1] {
                                let kind: CElementType = l.kind.into();
                                if let CElementType::Token(CTokenType::LeftParen) = kind {
                                    is_call = true;
                                }
                            }

                            if is_call {
                                let func = self.convert_tree_to_uir(
                                    builder,
                                    filtered_children[0].clone(),
                                    source,
                                    source_id,
                                );
                                let mut args = vec![];
                                for i in 2..filtered_children.len() {
                                    let child = &filtered_children[i];
                                    if let RedTree::Leaf(l) = child {
                                        let kind: CElementType = l.kind.into();
                                        if matches!(
                                            kind,
                                            CElementType::Token(CTokenType::RightParen)
                                                | CElementType::Token(CTokenType::Comma)
                                        ) {
                                            continue;
                                        }
                                    }
                                    args.push(self.convert_tree_to_uir(
                                        builder,
                                        child.clone(),
                                        source,
                                        source_id,
                                    ));
                                }
                                return builder.call(func, args, loc);
                            }
                        }

                        // Handle binary operations: [Left, Op, Right]
                        if filtered_children.len() == 3 {
                            if let RedTree::Leaf(op_leaf) = &filtered_children[1] {
                                let kind: CElementType = op_leaf.kind.into();
                                let op_text = match kind {
                                    CElementType::Token(CTokenType::Plus) => Some("+"),
                                    CElementType::Token(CTokenType::Minus) => Some("-"),
                                    CElementType::Token(CTokenType::Star) => Some("*"),
                                    CElementType::Token(CTokenType::Slash) => Some("/"),
                                    CElementType::Token(CTokenType::Assign) => Some("="),
                                    CElementType::Token(CTokenType::Equal) => Some("=="),
                                    CElementType::Token(CTokenType::NotEqual) => Some("!="),
                                    CElementType::Token(CTokenType::Less) => Some("<"),
                                    CElementType::Token(CTokenType::LessEqual) => Some("<="),
                                    CElementType::Token(CTokenType::Greater) => Some(">"),
                                    CElementType::Token(CTokenType::GreaterEqual) => Some(">="),
                                    _ => None,
                                };

                                if let Some(op) = op_text {
                                    let left = self.convert_tree_to_uir(
                                        builder,
                                        filtered_children[0].clone(),
                                        source,
                                        source_id,
                                    );
                                    let right = self.convert_tree_to_uir(
                                        builder,
                                        filtered_children[2].clone(),
                                        source,
                                        source_id,
                                    );
                                    return if op == "=" {
                                        builder.assign_to_id(left, right, loc)
                                    } else {
                                        builder.binary_op(op, left, right, loc)
                                    };
                                }
                            }

                            // If it's not a binary op but has 3 children, it might be something like (expr)
                            // or a wrapped expression. Let's try to find if it's a bracketed expression.
                            if let (RedTree::Leaf(l1), RedTree::Leaf(l3)) =
                                (&filtered_children[0], &filtered_children[2])
                            {
                                let k1: CElementType = l1.kind.into();
                                let k3: CElementType = l3.kind.into();
                                if let (
                                    CElementType::Token(CTokenType::LeftParen),
                                    CElementType::Token(CTokenType::RightParen),
                                ) = (k1, k3)
                                {
                                    return self.convert_tree_to_uir(
                                        builder,
                                        filtered_children[1].clone(),
                                        source,
                                        source_id,
                                    );
                                }
                            }
                        }

                        // If it's a single child (e.g., constant or identifier), or anything else
                        if filtered_children.len() == 1 {
                            return self.convert_tree_to_uir(
                                builder,
                                filtered_children[0].clone(),
                                source,
                                source_id,
                            );
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
                            let cond = self.convert_red_to_uir(
                                builder,
                                nodes[0].clone(),
                                source,
                                source_id,
                            );
                            let then_br = self.convert_red_to_uir(
                                builder,
                                nodes[1].clone(),
                                source,
                                source_id,
                            );
                            let else_br = if nodes.len() >= 3 {
                                self.convert_red_to_uir(
                                    builder,
                                    nodes[2].clone(),
                                    source,
                                    source_id,
                                )
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
                            let cond = self.convert_red_to_uir(
                                builder,
                                nodes[0].clone(),
                                source,
                                source_id,
                            );
                            let body = self.convert_red_to_uir(
                                builder,
                                nodes[1].clone(),
                                source,
                                source_id,
                            );
                            builder.while_loop(cond, body, loc)
                        } else {
                            builder.constant(0, loc)
                        }
                    }
                    _ => builder.constant(0, loc),
                }
            }
            RedTree::Leaf(leaf) => {
                let span = leaf.span;
                let text = &source[span.start..span.end];
                let loc = Loc::new(source_id, span.start as u32, span.end as u32);
                match leaf.kind.into() {
                    CElementType::Token(CTokenType::IntegerLiteral) => {
                        if let Ok(val) = text.parse::<i64>() {
                            builder.constant(val, loc)
                        } else {
                            builder.constant(0, loc)
                        }
                    }
                    CElementType::Token(CTokenType::FloatLiteral) => {
                        if let Ok(val) = text.parse::<f64>() {
                            builder.float(val, loc)
                        } else {
                            builder.constant(0, loc)
                        }
                    }
                    CElementType::Token(CTokenType::Identifier) => builder.symbol(text, loc),
                    CElementType::ExpressionStatement => {
                        // In Pratt parser, IntegerLiteral is often wrapped in ExpressionStatement directly
                        // Let's handle it here if it's a leaf
                        self.convert_tree_to_uir(builder, RedTree::Leaf(leaf), source, source_id)
                    }
                    _ => builder.constant(0, loc),
                }
            }
        }
    }
}
