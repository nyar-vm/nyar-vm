use nyar_hir::ast::{FunctionDefinition, FunctionParameter, FunctionParameterKind};

use super::*;

impl ParsingContext {
    pub(crate) fn parse_define(&mut self, pairs: &Token) -> Result<ASTNode> {
        let mut builder = FunctionDefinition::default();
        for pair in pairs {
            match pair.rule {
                Rule::COMMENT => continue,
                Rule::DEFINE => continue,
                Rule::Slash => continue,
                Rule::To => continue,
                Rule::define_parameter => self.define_parameter(&pair, &mut builder),
                Rule::Symbol => builder.push_symbol(self.parse_symbol(&pair)?, pair.span),
                Rule::block => builder.block = self.parse_block(&pair),
                Rule::MODIFIERS => builder.modifiers = self.parse_modifiers(&pair),
                Rule::type_expr => continue,
                Rule::effect_type => {
                    self.parse_effect_type(&pair);
                }
                _ => pair.debug_cases()?,
            }
        }
        Ok(builder.as_node(pairs.span))
    }
    fn define_parameter(&mut self, pairs: &Token, builder: &mut FunctionDefinition) {
        let mut mode = FunctionParameterKind::default();
        for item in pairs.filter_node(Rule::define_pair) {
            match self.define_pair(&item, &mut mode) {
                Ok(s) => builder.parameters.push(s),
                Err(e) => self.push_error(e),
            }
        }
    }
    fn define_pair(&mut self, pairs: &Token, mode: &mut FunctionParameterKind) -> Result<FunctionParameter> {
        let mut this_mode = *mode;
        let mut builder = FunctionParameter::default();
        for pair in pairs {
            match pair.rule {
                Rule::Set => continue,
                Rule::Symbol => builder.push_symbol(self.parse_symbol(&pair)?, pair.span),
                Rule::MODIFIERS => builder.modifiers = self.parse_modifiers(&pair),
                Rule::type_hint => continue,
                Rule::expr_inline => builder.default = Some(self.parse_expr(&pair)?),
                Rule::DEFINE_SPECIAL => {
                    match pair.text {
                        "<" => *mode = FunctionParameterKind::BothAvailable,
                        ">" => *mode = FunctionParameterKind::NamedOnly,
                        _ => {}
                    }
                    return Err(NyarError::msg(""));
                }
                Rule::DEFINE_MARK => match pair.text {
                    "^" => this_mode = FunctionParameterKind::Receiver,
                    "..." => this_mode = FunctionParameterKind::Deconstruct3,
                    ".." => this_mode = FunctionParameterKind::Deconstruct2,
                    _ => {}
                },
                _ => pair.debug_cases()?,
            }
        }
        builder.kind = this_mode;
        Ok(builder)
    }
}
