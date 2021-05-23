use crate::ast::{
    ApplyArgument, CallableItem, ChainCall, ContinuationArgument, ImportStatement, Operator, SliceArgument, SliceTerm,
    UnaryArgument,
};

use super::*;

// impl Display for SliceTerm {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         match self {
//             SliceTerm::Index { index } => write!(f, "[{}]", index),
//             SliceTerm::Slice { start, end, steps } => {
//                 write!(f, "[{}:{}:{}]", start, end, steps)
//             }
//         }
//     }
// }

impl VLanguage for ImportStatement {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(format!("({})", self))
    }
}

impl Display for ImportStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let alias = match self {
            ImportStatement::Symbol { name, alias } => {
                write!(f, "using {}", name)?;
                alias
            }
            ImportStatement::Script { path, name, alias } => {
                write!(f, "using {:?} {}", path, name)?;
                alias
            }
        };
        if let Some(alias) = alias {
            write!(f, " as {}", alias)?;
        }
        Ok(())
    }
}

impl VLanguage for ChainCall {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        let items = self.chain.iter().map(|item| item.v_format(arena));
        arena.hard_block("chain-call", items)
    }
}

impl VLanguage for CallableItem {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        match self {
            CallableItem::DotCall(v) => arena.inline_block("static-call", [v.v_format(arena)], " "),
            CallableItem::ApplyCall(v) => v.v_format(arena),
            CallableItem::SliceCall(v) => v.v_format(arena),
            CallableItem::UnaryCall(v) => v.v_format(arena),
            CallableItem::StaticCall(v) => arena.inline_block("static-call", [arena.as_string(v)], " "),
            CallableItem::BlockCall(v) => v.v_format(arena),
        }
    }
}

impl VLanguage for ApplyArgument {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.text("<<unimplemented ApplyArgument>>")
        // let items = self.items.iter().map(|item| item.v_format(arena));
        // let head = arena.text("(apply-call ");
        // let body = arena.intersperse(items, arena.line()).nest(1).group();
        // head.append(body).append(")")
    }
}

impl VLanguage for SliceArgument {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        let items = self.terms.iter().map(|item| item.v_format(arena));
        arena.hard_block("slice-call", items)
    }
}

impl VLanguage for SliceTerm {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        match self {
            SliceTerm::Index { index } => arena.hard_block("index", [index.v_format(arena)]),
            SliceTerm::Slice { start, end, steps } => {
                let start = match start {
                    Some(s) => s.v_format(arena),
                    None => arena.text("1"),
                };
                let end = match end {
                    Some(e) => e.v_format(arena),
                    None => arena.text("-1"),
                };
                let steps = match steps {
                    Some(s) => s.v_format(arena),
                    None => arena.text("1"),
                };
                arena.hard_block("slice", [start, end, steps])
            }
        }
    }
}

impl VLanguage for UnaryArgument {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.inline_block("suffix-call", self.suffix.iter().map(|f| f.v_format(arena)), ", ")
    }
}

impl VLanguage for Operator {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self)
    }
}

impl VLanguage for ContinuationArgument {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.text("<<unimplemented ContinuationArgument>>")
    }
}