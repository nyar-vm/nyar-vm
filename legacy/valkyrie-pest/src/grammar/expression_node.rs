use nyar_hir::ast::{ApplyArgument, ChainCall, SliceArgument, UnaryArgument};

use super::*;

impl ParsingContext {
    pub fn parse_term(&mut self, pairs: &Token) -> Result<ASTNode> {
        let mut chain = ChainCall::default();
        let mut unary = UnaryArgument::default();
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE | Rule::COMMENT => continue,
                Rule::node | Rule::node_inline => self.parse_node(&pair, &mut chain)?,
                Rule::Prefix => unary.push_prefix(pair.text),
                Rule::Suffix => unary.push_suffix(pair.text),
                _ => pair.debug_cases()?,
            };
        }
        chain += unary;
        Ok(chain.as_node(pairs.span))
    }

    fn parse_node(&mut self, pairs: &Token, chain: &mut ChainCall) -> Result<()> {
        let head = pairs.first()?;
        let head = match head.rule {
            Rule::expr => self.parse_expr(&head)?,
            Rule::data => self.parse_data(&head),
            _ => head.debug_cases()?,
        };
        chain.base = head;
        for pair in pairs.into_iter().skip(1) {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::callable => self.parse_callable(&pair, chain)?,
                Rule::block => chain.push_continuation(self.parse_block(&pair)),
                _ => pair.debug_cases()?,
            };
        }
        Ok(())
    }
    fn parse_callable(&mut self, pairs: &Token, chain: &mut ChainCall) -> Result<()> {
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::DOT => continue,
                Rule::apply => *chain += self.parse_apply(&pair),
                Rule::slice => *chain += self.parse_slice(&pair),
                Rule::dot_call => self.dot_call(&pair, chain)?,
                // Rule::block => *chain += self.parse_block(pair),
                _ => pair.debug_cases()?,
            };
        }
        Ok(())
    }
}

impl ParsingContext {
    pub(crate) fn parse_effect_type(&mut self, pairs: &Token) -> Vec<ASTNode> {
        let _ = pairs.filter_node(Rule::type_expr);
        vec![]
    }
}

impl ParsingContext {
    fn dot_call(&mut self, pairs: &Token, chain: &mut ChainCall) -> Result<()> {
        let node = pairs.first()?;
        match node.rule {
            Rule::namepath => chain.dot_symbol_call(self.parse_namepath(&node)?),
            Rule::Integer => chain.dot_number_call(self.parse_integer(&node)?, node.span),
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
    fn parse_apply(&mut self, pairs: &Token) -> ApplyArgument {
        let mut args = ApplyArgument::default();
        for pair in pairs.filter_node(Rule::apply_item) {
            match self.apply_item(&pair, &mut args) {
                Ok(_) => (),
                Err(e) => self.push_error(e),
            }
        }
        args
    }
    fn apply_item(&mut self, pairs: &Token, args: &mut ApplyArgument) -> Result<()> {
        let value = self.parse_expr(&pairs.last()?)?;
        match pairs.filter_node(Rule::Symbol).into_iter().next() {
            Some(s) => {
                let key = self.parse_symbol(&s)?;
                args.push_named(key, value);
            }
            None => args.push(value),
        }
        Ok(())
    }
}

impl ParsingContext {
    fn parse_slice(&mut self, pairs: &Token) -> SliceArgument {
        let mut args = SliceArgument::default();
        for pair in pairs.filter_node(Rule::index) {
            match self.parse_index(&pair, &mut args) {
                Ok(_) => (),
                Err(e) => self.push_error(e),
            }
        }
        args
    }
    fn parse_index(&mut self, pairs: &Token, args: &mut SliceArgument) -> Result<()> {
        let mut start = None;
        let mut end = None;
        let mut step = None;
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::COMMENT => continue,
                Rule::index_start => start = Some(self.parse_expr(&pair.first()?)?),
                Rule::index_end => end = Some(self.parse_expr(&pair.first()?)?),
                Rule::index_step => step = Some(self.parse_expr(&pair.first()?)?),
                Rule::expr => {
                    args.push_index(self.parse_expr(&pair)?);
                    return Ok(());
                }
                _ => pair.debug_cases()?,
            };
        }
        try { args.push_slice(start, end, step) }
    }
}
