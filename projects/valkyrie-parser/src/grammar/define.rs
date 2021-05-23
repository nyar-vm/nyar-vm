use super::*;

impl ParsingContext {
    pub(crate) fn parse_define(&self, pairs: Pair<Rule>) -> ASTNode {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::DEFINE => continue,
                Rule::define_symbol => {
                    self.define_symbol(pair);
                }
                Rule::define_parameter => {
                    self.define_parameter(pair);
                }
                _ => debug_cases!(pair),
            }
        }
        todo!()
    }
    fn define_symbol(&self, pairs: Pair<Rule>) {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::namepath => {
                    self.parse_namepath(pair);
                }
                _ => debug_cases!(pair),
            }
        }
    }
    fn define_parameter(&self, pairs: Pair<Rule>) {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::define_pair => continue,
                _ => debug_cases!(pair),
            }
        }
    }
}
