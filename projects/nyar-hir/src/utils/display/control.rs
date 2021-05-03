use crate::ast::{IfStatement, LoopStatement};

use super::*;

impl VLanguage for IfStatement {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        let mut items = vec![];
        for (cond, body) in &self.pairs {
            items.push(arena.hard_block("case", [cond.v_format(arena)]));
            items.push(arena.hard_block("then", body.iter().map(|item| item.v_format(arena))));
        }
        match &self.default {
            Some(s) => items.push(arena.hard_block("else", s.iter().map(|item| item.v_format(arena)))),
            None => items.push(arena.text("(else nothing)")),
        }
        arena.hard_block("switch", items)
    }
}

impl VLanguage for LoopStatement {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(format!("{:#?}", self))
    }
}
