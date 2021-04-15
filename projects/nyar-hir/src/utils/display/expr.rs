use super::*;

impl VLanguage for InfixCall {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        let name = format!("infix-call {}", self.operator);
        arena.hard_block(name, self.terms.iter().map(|item| item.v_format(arena)))
    }
}
