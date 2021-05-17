use nyar_hir::ast::{ForInLoop, IfStatement, WhileLoop};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_if(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut args = IfStatement::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::If | Rule::Else => continue,
                Rule::if_block => self.true_then(pair, &mut args),
                Rule::else_if_block => self.true_then(pair, &mut args),
                Rule::else_block => args.push_else(self.else_then(pair)),
                _ => debug_cases!(pair),
            }
        }
        args.as_node(r)
    }
    fn true_then(&mut self, pairs: Pair<Rule>, args: &mut IfStatement) {
        let mut pairs = pairs.into_inner();
        let block = unsafe { self.parse_block(pairs.next_back().unwrap_unchecked()) };
        let cond = unsafe { self.parse_expr(pairs.next_back().unwrap_unchecked()) };
        args.push_else_if(cond, block);
    }
    fn else_then(&mut self, pairs: Pair<Rule>) -> Vec<ASTNode> {
        let mut pairs = pairs.into_inner();
        unsafe { self.parse_block(pairs.next_back().unwrap_unchecked()) }
    }
}

impl ParsingContext {
    pub(crate) fn parse_while(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut args = WhileLoop::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::WHILE => continue,
                Rule::block => args.push_body(self.parse_block(pair)),
                Rule::expr_inline => args.push_condition(self.parse_expr(pair)),
                Rule::else_block => args.push_else(self.else_then(pair)),
                _ => debug_cases!(pair),
            }
        }
        args.as_node(r)
    }
}

impl ParsingContext {
    pub(crate) fn parse_for(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut args = ForInLoop::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::FOR => continue,
                // Rule::block => args.push_body(self.parse_block(pair)),
                // Rule::expr_inline => args.push_condition(self.parse_expr(pair)),
                // Rule::else_block => args.push_else(self.else_then(pair)),
                _ => debug_cases!(pair),
            }
        }
        args.as_node(r)
    }
}
