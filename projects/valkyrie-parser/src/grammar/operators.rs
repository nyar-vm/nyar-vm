use nyar_hir::ast::ImportBuilder;
use std::sync::LazyLock;

use super::*;

pub static PREC_CLIMBER: LazyLock<PrecClimber<Rule>> = LazyLock::new(|| {
    //TODO: use macro
    use crate::Rule::*;
    PrecClimber::new(vec![
        Operator::new(Set, Left),
        Operator::new(CONCAT, Left) | Operator::new(REMOVE, Left),
        Operator::new(ADD, Left) | Operator::new(SUB, Left),
        Operator::new(Multiply, Left) | Operator::new(CenterDot, Left),
        Operator::new(POWER, Right),
    ])
});

impl ParsingContext {
    pub fn parse_import(&mut self, pairs: Token) -> Vec<ASTNode> {
        let pair = unsafe { pairs.into_inner().next_back().unwrap_unchecked() };
        let mut builder = ImportBuilder::default();
        match pair.as_rule() {
            Rule::use_alias => self.use_alias(pair).as_node(r),
            Rule::module_block => {
                builder.push_block(self.module_block(pair));
                builder.as_node(r)
            }
            Rule::use_module_select => self.use_module_select(pair).as_node(r),
            Rule::use_module_string => self.use_module_string(pair).as_node(r),
            _ => pair.debug_cases()?,
        }
    }
    fn use_module_select(&self, pairs: Token) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::DOT | Rule::Proportion => continue,
                Rule::use_namepath => builder.push_symbol_path(self.parse_namepath(pair)),
                Rule::module_block => builder.push_block(self.module_block(pair)),
                _ => pair.debug_cases()?,
            }
        }
        builder
    }
    fn use_module_string(&mut self, pairs: Token) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::module_block => builder.push_block(self.module_block(pair)),
                Rule::String => builder.push_string_node(self.parse_string(pair)),
                _ => pair.debug_cases()?,
            }
        }
        builder
    }
    fn use_alias(&self, pairs: Token) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::AS => continue,
                Rule::DOT | Rule::Proportion => continue,
                Rule::use_namepath => builder.push_symbol_path(self.parse_namepath(pair)),
                Rule::Symbol => builder.push_alias(self.parse_symbol(pair)),
                _ => pair.debug_cases()?,
            }
        }
        builder
    }
    fn module_block(&self, pairs: Token) -> Vec<ImportBuilder> {
        let mut items = vec![];
        for pair in &pairs {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::use_alias => items.push(self.use_alias(pair)),
                Rule::use_module_select => items.push(self.use_module_select(pair)),
                _ => pair.debug_cases()?,
            }
        }
        return items;
    }
}
