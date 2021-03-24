use nyar_hir::ast::{ApplyArgument, ChainCall, SliceArgument, UnaryArgument};

use super::*;

impl ParsingContext {
    #[rustfmt::skip]
    pub(crate) fn parse_expr(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        // println!("{:#?}", pairs);
        PREC_CLIMBER.climb(
            pairs.into_inner().filter(|p| p.as_rule() != Rule::WHITESPACE),
            |pair: Pair<Rule>| match pair.as_rule() {
                // Rule::WHITESPACE => ASTNode::empty_statement(r),
                Rule::expr => self.parse_expr(pair),
                Rule::term => self.parse_term(pair),
                Rule::bracket_call => debug_cases!(pair),
                _ => debug_cases!(pair),
            },
            |left: ASTNode, op: Pair<Rule>, right: ASTNode| {
                left.push_infix_chain(op.as_str(), right, r)
            },
        )
    }

    fn parse_term(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut chain = ChainCall::default();
        let mut unary = UnaryArgument::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::COMMENT => continue,
                Rule::node => self.parse_node(pair, &mut chain),
                Rule::Prefix => unary.push_prefix(pair.as_str()),
                Rule::Suffix => unary.push_suffix(pair.as_str()),
                _ => unreachable!(),
            };
        }
        chain += unary;
        chain.as_node(r)
    }

    fn parse_node(&mut self, pairs: Pair<Rule>, chain: &mut ChainCall) {
        let r = self.get_span(&pairs);
        let mut pairs = pairs.into_inner();
        let head = pairs.next().unwrap();
        let head = match head.as_rule() {
            Rule::expr => self.parse_expr(head),
            Rule::data => self.parse_data(head),
            _ => unreachable!(),
        };
        chain.base = head;
        for pair in pairs {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::apply => *chain += self.parse_apply(pair),
                Rule::slice => *chain += self.parse_slice(pair),
                Rule::dot_call => unsafe {
                    let node = pair.into_inner().next_back().unwrap_unchecked();
                    debug_assert!(node.as_rule() == Rule::namepath);
                    *chain += self.parse_namepath(node)
                },
                _ => debug_cases!(pair),
            };
        }
    }
}

impl ParsingContext {
    fn parse_apply(&mut self, pairs: Pair<Rule>) -> ApplyArgument {
        let mut args = ApplyArgument::default();
        for pair in pairs.into_inner() {
            assert_eq!(pair.as_rule(), Rule::apply_item);
            match self.parse_apply_item(pair, &mut args) {
                Ok(_) => (),
                Err(e) => self.push_error(e),
            }
        }
        args
    }
    fn parse_apply_item(&mut self, pairs: Pair<Rule>, args: &mut ApplyArgument) -> Result<()> {
        let mut pairs = pairs.into_inner();
        let value = unsafe {
            let node = pairs.next_back().unwrap_unchecked();
            self.parse_expr(node)
        };
        match pairs.next_back() {
            Some(s) => {
                let key = self.parse_symbol(s);
                args.push_named(key, value);
            }
            None => args.push(value),
        }
        Ok(())
    }
}

impl ParsingContext {
    fn parse_slice(&mut self, pairs: Pair<Rule>) -> SliceArgument {
        let mut args = SliceArgument::default();
        let mut list = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::index => list.push(self.parse_index_term(pair)),
                _ => debug_cases!(pair),
            };
        }
        todo!()
        // ASTNode::apply_slice(&list, r)
    }
    fn parse_index_term(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let pair = pairs.into_inner().nth(0).unwrap();
        match pair.as_rule() {
            Rule::expr => self.parse_expr(pair),
            Rule::index_step => self.parse_index_step(pair),
            _ => debug_cases!(pair),
        }
    }

    fn parse_index_step(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Colon => continue,
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }
}
