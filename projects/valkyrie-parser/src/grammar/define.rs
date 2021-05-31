use nyar_hir::ast::{FunctionDefinition, FunctionParameter, FunctionParameterKind};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_define(&mut self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut builder = FunctionDefinition::default();
        for pair in pairs.into_inner() {
            let r = self.get_span(&pair);
            match pair.as_rule() {
                Rule::COMMENT => continue,
                Rule::DEFINE => continue,
                Rule::Slash => continue,
                Rule::To => continue,
                Rule::define_parameter => self.define_parameter(pair, &mut builder),
                Rule::Symbol => builder.push_symbol(self.parse_symbol(pair), r),
                Rule::block => builder.block = self.parse_block(pair),
                Rule::MODIFIERS => builder.modifiers = self.parse_modifiers(pair),
                Rule::type_expr => continue,
                Rule::effect_type => {
                    self.parse_effect_type(pair);
                }
                _ => debug_cases!(pair),
            }
        }
        builder.as_node(r)
    }
    fn define_parameter(&mut self, pairs: Pair<Rule>, builder: &mut FunctionDefinition) {
        let mut mode = FunctionParameterKind::default();
        for item in pairs.into_inner().filter(|f| f.as_rule() == Rule::define_pair) {
            match self.define_pair(item, &mut mode) {
                Some(s) => builder.parameters.push(s),
                None => {}
            }
        }
    }
    fn define_pair(&mut self, pairs: Pair<Rule>, mode: &mut FunctionParameterKind) -> Option<FunctionParameter> {
        let mut this_mode = *mode;
        let mut builder = FunctionParameter::default();
        for pair in pairs.into_inner() {
            let r = self.get_span(&pair);
            match pair.as_rule() {
                Rule::Set => continue,
                Rule::Symbol => builder.push_symbol(self.parse_symbol(pair), r),
                Rule::MODIFIERS => builder.modifiers = self.parse_modifiers(pair),
                Rule::type_hint => continue,
                Rule::expr_inline => builder.default = Some(self.parse_expr(pair)),
                Rule::DEFINE_SPECIAL => {
                    match pair.as_str() {
                        "<" => *mode = FunctionParameterKind::BothAvailable,
                        ">" => *mode = FunctionParameterKind::NamedOnly,
                        _ => {}
                    }
                    return None;
                }
                Rule::DEFINE_MARK => match pair.as_str() {
                    "^" => this_mode = FunctionParameterKind::Receiver,
                    "..." => this_mode = FunctionParameterKind::Deconstruct3,
                    ".." => this_mode = FunctionParameterKind::Deconstruct2,
                    _ => {}
                },
                _ => debug_cases!(pair),
            }
        }
        builder.kind = this_mode;
        Some(builder)
    }
}
