use nyar_hir::ast::{ApplyArgument, ChainCall, SliceArgument, UnaryArgument};

use crate::utils::union_node;

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
                Rule::term| Rule::term_inline => self.parse_term(pair),
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
                Rule::node | Rule::node_inline => self.parse_node(pair, &mut chain),
                Rule::Prefix => unary.push_prefix(pair.as_str()),
                Rule::Suffix => unary.push_suffix(pair.as_str()),
                _ => unreachable!(),
            };
        }
        chain += unary;
        chain.as_node(r)
    }

    fn parse_node(&mut self, pairs: Pair<Rule>, chain: &mut ChainCall) {
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
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::callable => self.parse_callable(pair, chain),
                Rule::block => chain.push_continuation(self.parse_block(pair)),
                _ => debug_cases!(pair),
            };
        }
    }
    fn parse_callable(&mut self, pairs: Pair<Rule>, chain: &mut ChainCall) {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::DOT => continue,
                Rule::apply => *chain += self.parse_apply(pair),
                Rule::slice => *chain += self.parse_slice(pair),
                Rule::dot_call => self.dot_call(pair, chain),
                // Rule::block => *chain += self.parse_block(pair),
                _ => debug_cases!(pair),
            };
        }
    }
}

impl ParsingContext {
    pub(crate) fn parse_effect_type(&mut self, pairs: Pair<Rule>) -> Vec<ASTNode> {
        let _ = pairs.into_inner().filter(|pair| pair.as_rule() == Rule::type_expr);
        vec![]
    }
}

impl ParsingContext {
    fn dot_call(&mut self, pairs: Pair<Rule>, chain: &mut ChainCall) {
        let r = self.get_span(&pairs);
        let node = union_node(pairs);
        match node.as_rule() {
            Rule::namepath => chain.dot_symbol_call(self.parse_namepath(node)),
            Rule::Integer => chain.dot_number_call(self.parse_integer(node), r),
            // Rule::WHITESPACE | Rule::DOT => continue,
            // Rule::COMMENT => continue,
            // Rule::apply => *chain += self.parse_apply(pair),
            // Rule::slice => *chain += self.parse_slice(pair),
            // Rule::dot_call => unsafe {
            //     let node = pair.into_inner().next_back().unwrap_unchecked();
            //     debug_assert!(node.as_rule() == Rule::namepath);
            //     *chain += self.parse_namepath(node)
            // },
            // Rule::block => *chain += self.parse_block(pair),
            _ => debug_cases!(node),
        };
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
        for pair in pairs.into_inner() {
            assert_eq!(pair.as_rule(), Rule::index);
            self.parse_index(pair, &mut args);
        }
        args
    }
    fn parse_index(&mut self, pairs: Pair<Rule>, args: &mut SliceArgument) {
        let mut start = None;
        let mut end = None;
        let mut step = None;
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::index_start => start = Some(self.parse_expr(union_node(pair))),
                Rule::index_end => end = Some(self.parse_expr(union_node(pair))),
                Rule::index_step => step = Some(self.parse_expr(union_node(pair))),
                Rule::expr => {
                    args.push_index(self.parse_expr(pair));
                    return;
                }
                _ => debug_cases!(pair),
            };
        }
        args.push_slice(start, end, step)
    }
}
