use crate::ast::{
    ApplyArgument, CallableItem, ChainCall, ContinuationArgument, Operator, SliceArgument, SliceTerm, UnaryArgument,
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

impl VLanguage for ChainCall {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        let items = self.chain.iter().map(|item| item.v_format(arena));
        arena.hard_block("chain-call", items)
    }
}

impl VLanguage for CallableItem {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        match self {
            CallableItem::DotCall(v) => arena.inline_block("static-call", [v.v_format(arena)]),
            CallableItem::ApplyCall(v) => v.v_format(arena),
            CallableItem::SliceCall(v) => v.v_format(arena),
            CallableItem::UnaryCall(v) => v.v_format(arena),
            CallableItem::StaticCall(v) => arena.inline_block("static-call", [arena.as_string(v)]),
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
        arena.text("<<unimplemented SliceArgument>>")
        // let prefix: Vec<_> = self.prefix.iter().map(|f| f.to_string()).collect();
        // let suffix: Vec<_> = self.suffix.iter().map(|f| f.to_string()).collect();
        // writeln!(f, "(suffix {})", prefix.join(", "))?;
        // writeln!(f, "(prefix {})", suffix.join(", "))

        // let items = self.items.iter().map(|item| item.v_format(arena));
        // let head = arena.text("(unary-call ");
        // let body = arena.intersperse(items, arena.line()).nest(1).group();
        // head.append(body).append(")")
    }
}

impl VLanguage for UnaryArgument {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.inline_block("suffix-call", self.suffix.iter().map(|f| f.v_format(arena)))
    }
}

impl VLanguage for Operator {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.as_string(self.to_string())
    }
}

impl VLanguage for ContinuationArgument {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.text("<<unimplemented ContinuationArgument>>")
    }
}
