use nyar_hir::ast::{ApplyArgument, ChainCall, SliceArgument, UnaryArgument};

use super::*;

impl ParsingContext {
    #[rustfmt::skip]
    pub(crate) fn parse_expr(&mut self, pairs: Token) -> Result<ASTNode> {
        let r = pairs.span;
        // println!("{:#?}", token);
        PREC_CLIMBER.climb(
            pairs.into_inner().filter(|p| p.as_rule() != Rule::WHITESPACE),
            |pair: Pair<Rule>| match pair.as_rule() {
                Rule::expr => self.parse_expr(pair),
                Rule::term| Rule::term_inline => self.parse_term(pair),
                _ => pair.debug_cases()?,
            },
            |left: ASTNode, op: Pair<Rule>, right: ASTNode| {
                left.push_infix_chain(op.as_str(), right, r)
            },
        )
    }

    fn parse_term(&mut self, pairs: Token) -> ASTNode {
        let mut chain = ChainCall::default();
        let mut unary = UnaryArgument::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::COMMENT => continue,
                Rule::node | Rule::node_inline => self.parse_node(pair, &mut chain),
                Rule::Prefix => unary.push_prefix(pair.text),
                Rule::Suffix => unary.push_suffix(pair.text),
                _ => unreachable!(),
            };
        }
        chain += unary;
        chain.as_node(r)
    }

    fn parse_node(&mut self, pairs: Token, chain: &mut ChainCall) {
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
                _ => pair.debug_cases()?,
            };
        }
    }
    fn parse_callable(&mut self, pairs: Token, chain: &mut ChainCall) {
        for pair in &pairs {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::DOT => continue,
                Rule::apply => *chain += self.parse_apply(pair),
                Rule::slice => *chain += self.parse_slice(pair),
                Rule::dot_call => self.dot_call(pair, chain),
                // Rule::block => *chain += self.parse_block(pair),
                _ => pair.debug_cases()?,
            };
        }
    }
}

impl ParsingContext {
    pub(crate) fn parse_effect_type(&mut self, pairs: Token) -> Vec<ASTNode> {
        let _ = pairs.into_inner().filter(|pair| pair.as_rule() == Rule::type_expr);
        vec![]
    }
}

impl ParsingContext {
    fn dot_call(&mut self, pairs: Token, chain: &mut ChainCall) -> Result<()> {
        let node = pairs.first()?;
        match node.rule {
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
            _ => node.debug_cases()?,
        };
        Ok(())
    }
}

impl ParsingContext {
    fn parse_apply(&mut self, pairs: Token) -> Result<ApplyArgument> {
        let mut args = ApplyArgument::default();
        for pair in &pairs {
            assert_eq!(pair.as_rule(), Rule::apply_item);
            match self.parse_apply_item(pair, &mut args) {
                Ok(_) => (),
                Err(e) => self.push_error(e),
            }
        }
        Ok(args)
    }
    fn parse_apply_item(&mut self, pairs: Token, args: &mut ApplyArgument) -> Result<()> {
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
    fn parse_slice(&mut self, pairs: Token) -> Result<SliceArgument> {
        let mut args = SliceArgument::default();
        for pair in &pairs.filter_node() {
            assert_eq!(pair.as_rule(), Rule::index);
            self.parse_index(pair, &mut args);
        }
        args
    }
    fn parse_index(&mut self, pairs: Token, args: &mut SliceArgument) -> Result<()> {
        let mut start = None;
        let mut end = None;
        let mut step = None;
        for pair in &pairs {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::index_start => start = Some(self.parse_expr(pair.first()?)),
                Rule::index_end => end = Some(self.parse_expr(pair.first()?)),
                Rule::index_step => step = Some(self.parse_expr(pair.first()?)),
                Rule::expr => {
                    args.push_index(self.parse_expr(pair));
                    return Ok(());
                }
                _ => pair.debug_cases()?,
            };
        }
        try { args.push_slice(start, end, step) }
    }
}
