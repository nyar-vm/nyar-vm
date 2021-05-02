use crate::ast::{KVPair, StringLiteral, TableExpression};

use super::*;

impl VLanguage for Symbol {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self)
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for s in &self.scope {
            write_identifier(s, f)?;
            f.write_str("::")?;
        }
        write_identifier(&self.name, f)
    }
}

fn write_identifier(id: &str, f: &mut Formatter) -> std::fmt::Result {
    match is_valid_identifier(id) {
        true => write!(f, "{}", id),
        false => write!(f, "`{}`", id),
    }
}

fn is_valid_identifier(id: &str) -> bool {
    id.chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl VLanguage for IntegerLiteral {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self)
    }
}

impl Display for IntegerLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.handler.is_empty() {
            true => write!(f, "{}", self.value),
            false => write!(f, "{}_{}", self.value, self.handler),
        }
    }
}

impl VLanguage for DecimalLiteral {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self)
    }
}

impl Display for DecimalLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)?;
        if !self.handler.is_empty() {
            write!(f, "_{}", self.handler)?;
        }
        Ok(())
    }
}

impl VLanguage for ByteLiteral {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.inline_block("byte-literal", [arena.as_string(self.handler), arena.as_string(&self.value)].into_iter(), " ")
    }
}

impl Display for ByteLiteral {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "0{}{}", self.handler, self.value)
    }
}

impl VLanguage for StringLiteral {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self)
    }
}

impl Display for StringLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{:?}", self.handler, self.literal)
    }
}

impl VLanguage for TableExpression {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        if self.inner.is_empty() {
            return arena.text("(table-literal nothing)");
        }
        arena.hard_block("table-literal", self.inner.iter().map(|f| f.v_format(arena)))
    }
}

impl VLanguage for KVPair {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.hard_block("pair", [self.key.v_format(arena), self.value.v_format(arena)].into_iter())
    }
}
