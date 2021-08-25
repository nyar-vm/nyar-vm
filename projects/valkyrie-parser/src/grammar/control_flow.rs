use nyar_hir::ast::{ForInLoop, IfStatement, WhileLoop};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_if(&mut self, pairs: &Token) -> Result<ASTNode> {
        let mut args = IfStatement::default();
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::IF | Rule::ELSE => continue,
                Rule::if_block => self.true_then(&pair, &mut args)?,
                Rule::else_if_block => self.true_then(&pair, &mut args)?,
                Rule::else_block => args.push_else(self.else_then(&pair)?),
                _ => pair.debug_cases()?,
            }
        }
        Ok(args.as_node(pairs.span))
    }
    fn true_then(&mut self, pairs: &Token, args: &mut IfStatement) -> Result<()> {
        let block = self.parse_block(&pairs.nth(-1)?);
        let cond = self.parse_expr(&pairs.nth(-2)?)?;
        Ok(args.push_else_if(cond, block))
    }
    fn else_then(&mut self, pairs: &Token) -> Result<Vec<ASTNode>> {
        Ok(self.parse_block(&pairs.last()?))
    }
}

impl ParsingContext {
    pub(crate) fn parse_while(&mut self, pairs: &Token) -> Result<ASTNode> {
        let mut args = WhileLoop::default();
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::WHILE => continue,
                Rule::block => args.push_body(self.parse_block(&pair)),
                Rule::expr_inline => args.push_condition(self.parse_expr(&pair)?),
                Rule::else_block => args.push_else(self.else_then(&pair)?),
                _ => pair.debug_cases()?,
            }
        }
        try { args.as_node(pairs.span) }
    }
}

impl ParsingContext {
    pub(crate) fn parse_for(&mut self, pairs: &Token) -> Result<ASTNode> {
        let mut args = ForInLoop::default();
        for pair in pairs {
            match pair.rule {
                Rule::WHITESPACE => continue,
                Rule::FOR => continue,
                // Rule::block => args.push_body(self.parse_block(pair)),
                // Rule::expr_inline => args.push_condition(self.parse_expr(pair)),
                // Rule::else_block => args.push_else(self.else_then(pair)),
                _ => pair.debug_cases()?,
            }
        }
        try { args.as_node(pairs.span) }
    }
}
