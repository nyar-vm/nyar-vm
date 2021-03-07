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
        let mut base = ASTNode::default();
        let mut prefix = vec![];
        let mut suffix = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::COMMENT => continue,
                Rule::node => base = self.parse_node(pair),
                Rule::Prefix => prefix.push(pair.as_str().to_string()),
                Rule::Suffix => suffix.push(pair.as_str().to_string()),
                _ => unreachable!(),
            };
        }
        base.push_unary_operations(&prefix, &suffix, r)
    }

    fn parse_node(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let mut pairs = pairs.into_inner();
        let head = pairs.next().unwrap();
        let head = match head.as_rule() {
            Rule::expr => self.parse_expr(head),
            Rule::data => self.parse_data(head),
            _ => unreachable!(),
        };
        let head = ChainBuilder::new(head);
        for pair in pairs {
            return match pair.as_rule() {
                Rule::apply => self.parse_apply(pair),
                // Rule::bracket_call => self.parse_bracket_call(pair),
                // Rule::tuple => self.parse_list_or_tuple(pair, false),
                _ => debug_cases!(pair),
            };
        }
        return head.as_node();
    }

    fn parse_bracket_call(&mut self, pairs: Pair<Rule>) -> ASTNode {
        // let r = self.get_span(&pairs);
        let mut base = ASTNode::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::data => base = self.parse_data(pair),
                Rule::apply => base = ASTNode::chain_join(base, self.parse_apply(pair)),
                Rule::slice => base = ASTNode::chain_join(base, self.parse_slice(pair)),
                Rule::dot_call => continue,
                Rule::block => continue,
                _ => debug_cases!(pair),
            };
        }
        return base;
    }

    fn parse_slice(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut list = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::index => list.push(self.parse_index_term(pair)),
                _ => debug_cases!(pair),
            };
        }
        ASTNode::apply_slice(&list, r)
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

impl ParsingContext {
    fn parse_apply(&self, pairs: Pair<Rule>) -> ASTNode {

        Argu
        pairs.into_inner().filter(|f| f.as_rule() == Rule::apply_kv).map(|f| self.apply_kv(f)).collect()

        let r = self.get_span(&pairs);
        let mut args = vec![];
        // let mut kv_pairs = vec![];
        // let mut types = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                // Rule::apply_kv => args.push(self.parse_kv(pair)),
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::apply_call(args, r);
    }
    fn apply_kv(&self, pairs: Pair<Rule>) {

    }
}
