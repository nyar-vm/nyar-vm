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
    pub fn parse_import(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let pair = unsafe { pairs.into_inner().next_back().unwrap_unchecked() };
        match pair.as_rule() {
            Rule::use_alias => self.use_alias(pair).as_node(r),
            Rule::module_block => ImportBuilder::push_symbol_path(self.module_block(pair)).as_node(r),
            Rule::use_module_select => self.use_module_select(pair).as_node(r),
            Rule::use_module_string => self.use_module_string(pair).as_node(r),
            _ => debug_cases!(pair),
        }
    }
    fn use_module_select(&self, pairs: Pair<Rule>) -> ImportBuilder {
        let mut pairs = pairs.into_inner();
        let path = unsafe {
            let pair = pairs.next().unwrap_unchecked();
            debug_assert!(pair.as_rule() == Rule::use_namepath);
            self.parse_namepath(pair)
        };
        match pairs.next_back() {
            Some(s) => {
                debug_assert!(s.as_rule() == Rule::module_block);
                ImportBuilder::push_string_path(path, self.module_block(s))
            }
            None => ImportBuilder::push_string_path(path, vec![]),
        }
    }
    fn use_module_string(&mut self, pairs: Pair<Rule>) -> ImportBuilder {
        let mut pairs = pairs.into_inner();
        let path = unsafe { pairs.next().unwrap().as_str().to_string() };
        for pair in pairs {
            match pair.as_rule() {
                Rule::module_block => continue,
                _ => debug_cases!(pair),
            }
        }
        return ImportBuilder::StringBlock(path, vec![]);
    }
    fn use_alias(&self, pairs: Pair<Rule>) -> ImportBuilder {
        let mut symbol = Symbol::default();
        let mut alias = None;
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::DOT => continue,
                Rule::AS => continue,
                Rule::use_namepath => symbol = self.parse_namepath(pair),
                Rule::Symbol => alias = Some(self.parse_symbol(pair)),
                _ => debug_cases!(pair),
            }
        }
        return ImportBuilder::push_block(symbol, alias);
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
