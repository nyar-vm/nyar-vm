use super::*;
use crate::ast::FunctionDefinition;

impl VLanguage for FunctionDefinition {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(format!("{:#?}", self))
    }
}
