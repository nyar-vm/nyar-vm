use nyar_hir::ast::DefineBuilder;

use super::*;

impl ParsingContext {
    pub(crate) fn parse_define(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut builder = DefineBuilder::default();
        for pair in pairs.into_inner() {
            let r = self.get_span(&pair);
            match pair.as_rule() {
                Rule::DEFINE => continue,
                Rule::define_parameter => self.define_parameter(pair, &mut builder),
                Rule::Symbol => builder.push_symbol(self.parse_symbol(pair), r),
                Rule::block => builder.push_block(self.parse_block(pair), r),
                _ => debug_cases!(pair),
            }
        }
        builder.as_node(r)
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
    fn define_parameter(&self, pairs: Pair<Rule>, builder: &mut DefineBuilder) {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::define_pair => continue,
                _ => debug_cases!(pair),
            }
        }
    }
}
