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
        match self.try_parse_import(&pairs) {
            Ok(o) => o.as_node(pairs.span),
            Err(_) => {
                vec![]
            }
        }
    }

    fn try_parse_import(&mut self, pairs: &Token) -> Result<ImportBuilder> {
        let pair = pairs.last()?;
        let mut builder = ImportBuilder::default();
        match pair.rule {
            Rule::use_alias => self.use_alias(pair),
            Rule::module_block => {
                builder.push_block(self.module_block(pair));
                builder
            }
            Rule::use_module_select => self.use_module_select(pair),
            Rule::use_module_string => self.use_module_string(pair),
            _ => pair.debug_cases()?,
        }
    }
    fn use_module_select(&self, pairs: Token) -> Result<ImportBuilder> {
        let mut builder = ImportBuilder::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::DOT | Rule::Proportion => continue,
                Rule::use_namepath => builder.push_symbol_path(self.parse_namepath(pair)?),
                Rule::module_block => builder.push_block(self.module_block(pair)?),
                _ => pair.debug_cases()?,
            }
        }
        Ok(builder)
    }
    fn use_module_string(&mut self, pairs: Token) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::module_block => builder.push_block(self.module_block(pair)?),
                Rule::String => builder.push_string_node(self.parse_string(pair)),
                _ => pair.debug_cases()?,
            }
        }
        builder
    }
    fn use_alias(&self, pairs: Token) -> Result<ImportBuilder> {
        let mut builder = ImportBuilder::default();
        for pair in &pairs {
            match pair.as_rule() {
                Rule::AS => continue,
                Rule::DOT | Rule::Proportion => continue,
                Rule::use_namepath => builder.push_symbol_path(self.parse_namepath(pair)?),
                Rule::Symbol => builder.push_alias(self.parse_symbol(pair)?),
                _ => pair.debug_cases()?,
            }
        }
        Ok(builder)
    }
    fn module_block(&self, pairs: Token) -> Result<Vec<ImportBuilder>> {
        let mut items = vec![];
        for pair in &pairs {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::use_alias => items.push(self.use_alias(pair)?),
                Rule::use_module_select => items.push(self.use_module_select(pair)?),
                _ => pair.debug_cases()?,
            }
        }
        Ok(items)
    }
}
