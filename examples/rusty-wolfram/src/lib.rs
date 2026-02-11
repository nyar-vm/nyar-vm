use nyar_types::{NyarContext, NyarError, NyarFrontend, Id, Vfs, Loc};
use oak_wolfram::{WolframBuilder, WolframLanguage, ast::WolframRoot};
use oak_core::{Builder, SourceText, tree::TypedNode};

pub struct RustyWolframFrontend {
    language: WolframLanguage,
}

impl RustyWolframFrontend {
    pub fn new() -> Self {
        Self {
            language: WolframLanguage::default(),
        }
    }
}

impl Default for RustyWolframFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl NyarFrontend for RustyWolframFrontend {
    type Language = WolframLanguage;

    fn parse(&self, source: &str) -> Result<WolframRoot, NyarError> {
        let builder = WolframBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<WolframLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &WolframRoot, ctx: &mut NyarContext<V>) -> Id {
        let mut stmts = Vec::new();
        for node in ast.green().children() {
            if let Some(expr_id) = self.lower_node(node, ctx) {
                stmts.push(expr_id);
            }
        }
        ctx.builder().module("rusty-wolfram-program", stmts, Loc::default())
    }
}

impl RustyWolframFrontend {
    fn lower_node<V: Vfs>(
        &self,
        tree: &oak_core::tree::GreenTree<WolframLanguage>,
        ctx: &mut NyarContext<V>,
    ) -> Option<Id> {
        use oak_wolfram::ast::*;
        let node = tree.as_node()?;
        let red = oak_core::tree::RedNode::new(node, 0);

        if let Some(symbol) = WolframSymbol::cast(red.clone()) {
            let name = symbol.green().text().to_string();
            // Handle slots specifically if needed, or just treat them as symbols
            return Some(ctx.builder().symbol(&name, Loc::default()));
        }

        if let Some(literal) = WolframLiteral::cast(red.clone()) {
            let text = literal.green().text();
            if let Ok(val) = text.parse::<i64>() {
                return Some(ctx.builder().int(val, Loc::default()));
            }
            if let Ok(val) = text.parse::<f64>() {
                return Some(ctx.builder().float(val, Loc::default()));
            }
            // Strip quotes from string literals
            let content = if text.starts_with('"') && text.ends_with('"') {
                &text[1..text.len() - 1]
            } else {
                text
            };
            return Some(ctx.builder().string(content, Loc::default()));
        }

        if let Some(expr) = WolframExpression::cast(red.clone()) {
            for child in expr.green().children() {
                if let Some(id) = self.lower_node(child, ctx) {
                    return Some(id);
                }
            }
        }

        if let Some(call) = WolframCall::cast(red.clone()) {
            let mut children = call.green().children().iter();
            if let Some(head_tree) = children.next() {
                let head_id = self.lower_node(head_tree, ctx)?;
                let mut args = Vec::new();
                for next_tree in children {
                    if let Some(node) = next_tree.as_node() {
                        if let Some(wolfram_args) = WolframArguments::cast(oak_core::tree::RedNode::new(node, 0)) {
                            for arg_tree in wolfram_args.green().children() {
                                if let Some(arg_id) = self.lower_node(arg_tree, ctx) {
                                    args.push(arg_id);
                                }
                            }
                        }
                    }
                }
                return Some(ctx.builder().call(head_id, args, Loc::default()));
            }
        }

        if let Some(binary) = WolframBinaryExpr::cast(red.clone()) {
            let mut children = binary.green().children().iter();
            if let (Some(left_tree), Some(op_tree), Some(right_tree)) = (children.next(), children.next(), children.next()) {
                let left_id = self.lower_node(left_tree, ctx)?;
                let right_id = self.lower_node(right_tree, ctx)?;
                let op_text = op_tree.as_node().map(|n| n.text()).unwrap_or_default();

                return match op_text {
                    "+" => Some(ctx.builder().add_op(left_id, right_id, Loc::default())),
                    "-" => Some(ctx.builder().sub_op(left_id, right_id, Loc::default())),
                    "*" => Some(ctx.builder().mul_op(left_id, right_id, Loc::default())),
                    "/" => Some(ctx.builder().div_op(left_id, right_id, Loc::default())),
                    "^" => Some(ctx.builder().binary_op("pow", left_id, right_id, Loc::default())),
                    "&&" => Some(ctx.builder().binary_op("and", left_id, right_id, Loc::default())),
                    "||" => Some(ctx.builder().binary_op("or", left_id, right_id, Loc::default())),
                    "==" => Some(ctx.builder().eq_op(left_id, right_id, Loc::default())),
                    "!=" => Some(ctx.builder().ne_op(left_id, right_id, Loc::default())),
                    "<" => Some(ctx.builder().lt_op(left_id, right_id, Loc::default())),
                    ">" => Some(ctx.builder().gt_op(left_id, right_id, Loc::default())),
                    "<=" => Some(ctx.builder().le_op(left_id, right_id, Loc::default())),
                    ">=" => Some(ctx.builder().ge_op(left_id, right_id, Loc::default())),
                    "->" | "\[Rule]" => Some(ctx.builder().extension("rule", vec![left_id, right_id], Loc::default())),
                    ":>" | "\[RuleDelayed]" => Some(ctx.builder().extension("rule_delayed", vec![left_id, right_id], Loc::default())),
                    "=" => Some(ctx.builder().assign_to_id(left_id, right_id, Loc::default())),
                    ":=" => Some(ctx.builder().extension("set_delayed", vec![left_id, right_id], Loc::default())),
                    "@" => Some(ctx.builder().call(left_id, vec![right_id], Loc::default())),
                    "/@" => Some(ctx.builder().map(left_id, right_id, Loc::default())),
                    "@@" => Some(ctx.builder().extension("apply", vec![left_id, right_id], Loc::default())),
                    "@@@" => Some(ctx.builder().extension("apply_level", vec![left_id, right_id], Loc::default())),
                    "//@" => Some(ctx.builder().extension("map_all", vec![left_id, right_id], Loc::default())),
                    "//" => Some(ctx.builder().call(right_id, vec![left_id], Loc::default())),
                    _ => Some(ctx.builder().binary_op(op_text, left_id, right_id, Loc::default())),
                };
            }
        }

        if let Some(prefix) = WolframPrefixExpr::cast(red.clone()) {
            let mut children = prefix.green().children().iter();
            if let (Some(op_tree), Some(arg_tree)) = (children.next(), children.next()) {
                let arg_id = self.lower_node(arg_tree, ctx)?;
                let op_text = op_tree.as_node().map(|n| n.text()).unwrap_or_default();
                return match op_text {
                    "-" => Some(ctx.builder().neg_op(arg_id, Loc::default())),
                    "!" => Some(ctx.builder().not_op(arg_id, Loc::default())),
                    _ => None,
                };
            }
        }

        if let Some(postfix) = WolframPostfixExpr::cast(red.clone()) {
            let mut children = postfix.green().children().iter();
            if let (Some(arg_tree), Some(op_tree)) = (children.next(), children.next()) {
                let arg_id = self.lower_node(arg_tree, ctx)?;
                let op_text = op_tree.as_node().map(|n| n.text()).unwrap_or_default();
                return match op_text {
                    "!" => Some(ctx.builder().extension("factorial", vec![arg_id], Loc::default())),
                    "&" => {
                        // body & -> pure function
                        // We need to extract parameters (#, ##, #1, #2...) from body
                        // For now, let's assume a simple case
                        Some(ctx.builder().lambda(vec!["#".to_string()], arg_id, Loc::default()))
                    }
                    _ => None,
                };
            }
        }

        if let Some(list) = WolframList::cast(red.clone()) {
            let mut items = Vec::new();
            for item_tree in list.green().children() {
                if let Some(item_id) = self.lower_node(item_tree, ctx) {
                    items.push(item_id);
                }
            }
            return Some(ctx.builder().extension("list", items, Loc::default()));
        }

        None
    }
}
