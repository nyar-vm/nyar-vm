use nyar_hir::ast::{FunctionDefinition, FunctionParameter, FunctionParameterKind, SymbolNode};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_define(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut builder = FunctionDefinition::default();
        for pair in pairs.into_inner() {
            let r = self.get_span(&pair);
            match pair.as_rule() {
                Rule::DEFINE => continue,
                Rule::define_parameter => self.define_parameter(pair, &mut builder),
                Rule::Symbol => builder.push_symbol(self.parse_symbol(pair), r),
                Rule::block => builder.block = self.parse_block(pair),
                Rule::MODIFIERS => builder.modifiers = self.parse_modifiers(pair),
                Rule::return_type => {
                    self.return_type(pair, &mut builder);
                }
                Rule::return_effect => {
                    self.return_effect(pair, &mut builder);
                }
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
    fn define_parameter(&self, pairs: Pair<Rule>, builder: &mut FunctionDefinition) {
        let mut mode = FunctionParameterKind::default();
        for item in pairs.into_inner().filter(|f| f.as_rule() == Rule::define_pair).map(|f| self.define_pair(f)) {
            self.define_pair(i, mode)
        }
    }
    fn return_type(&mut self, pairs: Pair<Rule>, builder: &mut FunctionDefinition) {
        let _ = pairs.into_inner().filter(|pair| pair.as_rule() == Rule::type_expr).next().unwrap();
    }
    fn return_effect(&mut self, pairs: Pair<Rule>, builder: &mut FunctionDefinition) {
        let _ = pairs.into_inner().filter(|pair| pair.as_rule() == Rule::type_expr);
    }
    fn define_pair(&self, pairs: Pair<Rule>, mode: &mut FunctionParameterKind) -> Option<FunctionParameter> {
        let mut builder = FunctionParameter::default();
        for pair in pairs.into_inner() {
            let r = self.get_span(&pair);
            match pair.as_rule() {
                Rule::DEFINE_SPECIAL => builder.push_special(pair.as_str(), mode, r),
                _ => debug_cases!(pair),
            }
        }
        match builder.is_delimiter() {
            true => None,
            false => Some(builder),
        }
    }
}
