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
    pub fn parse_import(&self, pairs: Pair<Rule>) -> ASTNode {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::IMPORT => continue,
                Rule::use_module_select => return self.use_module_select(pair),
                _ => debug_cases!(pair),
            }
        }
        unimplemented!()
    }
    fn use_module_select(&self, pairs: Pair<Rule>) -> ASTNode {
        let mut symbol = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Symbol => symbol.push(self.parse_symbol(pair)),
                _ => debug_cases!(pair),
            }
        }
        return Symbol::join(symbol);
    }
    fn use_module_select2(&self, pairs: Pair<Rule>) -> ASTNode {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                _ => debug_cases!(pair),
            }
        }
        unimplemented!()
    }
}
