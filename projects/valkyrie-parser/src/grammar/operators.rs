use nyar_hir::ast::ImportBuilder;

use super::*;

pub static PREC_CLIMBER: SyncLazy<PrecClimber<Rule>> = SyncLazy::new(|| {
    //TODO: use macro
    use crate::Rule::*;
    PrecClimber::new(vec![
        Operator::new(Set, Left),
        Operator::new(CONCAT, Left) | Operator::new(REMOVE, Left),
        Operator::new(ADD, Left) | Operator::new(SUB, Left),
        Operator::new(Multiply, Left) | Operator::new(CenterDot, Left),
        Operator::new(Power, Right),
    ])
});

impl ParsingContext {
    pub fn parse_import(&mut self, pairs: Pair<Rule>) -> Vec<ASTNode> {
        let r = self.get_span(&pairs);
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
            _ => debug_cases!(pair),
        }
    }
    fn use_module_select(&self, pairs: Pair<Rule>) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::DOT => continue,
                Rule::use_namepath => builder.push_symbol_path(self.parse_namepath(pair)),
                Rule::module_block => builder.push_block(self.module_block(pair)),
                _ => debug_cases!(pair),
            }
        }
        builder
    }
    fn use_module_string(&mut self, pairs: Pair<Rule>) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::module_block => builder.push_block(self.module_block(pair)),
                Rule::String => builder.push_string_node(self.parse_string(pair)),
                _ => debug_cases!(pair),
            }
        }
        builder
    }
    fn use_alias(&self, pairs: Pair<Rule>) -> ImportBuilder {
        let mut builder = ImportBuilder::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::DOT => continue,
                Rule::AS => continue,
                Rule::use_namepath => builder.push_symbol_path(self.parse_namepath(pair)),
                Rule::Symbol => builder.push_alias(self.parse_symbol(pair)),
                _ => debug_cases!(pair),
            }
        }
        builder
    }
    fn module_block(&self, pairs: Pair<Rule>) -> Vec<ImportBuilder> {
        let mut items = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::use_alias => items.push(self.use_alias(pair)),
                Rule::use_module_select => items.push(self.use_module_select(pair)),
                _ => debug_cases!(pair),
            }
        }
        return items;
    }
}
