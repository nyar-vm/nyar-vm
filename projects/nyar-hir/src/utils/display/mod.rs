use std::{
    fmt::{Display, Formatter},
    ops::Deref,
};

use pretty::{Arena, DocAllocator, DocBuilder, Pretty};

use nyar_error::Result;

use crate::{
    ast::{ByteLiteral, DecimalLiteral, InfixCall, IntegerLiteral, Symbol},
    ASTKind, ASTNode,
};

mod atom;
mod call;
mod expr;

impl ASTNode {
    pub fn pretty_print(&self, width: usize) -> Result<String> {
        self.kind.pretty_print(width)
    }
}

impl ASTKind {
    pub fn pretty_print(&self, width: usize) -> Result<String> {
        let mut out = String::new();
        let fmt = PrettyFormatter { arena: &Default::default() };
        self.v_format(&fmt).render_fmt(width, &mut out)?;
        Ok(out)
    }
}

struct PrettyFormatter<'a> {
    arena: &'a Arena<'a>,
}

impl<'a> Deref for PrettyFormatter<'a> {
    type Target = &'a Arena<'a>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl<'a> PrettyFormatter<'a> {
    pub fn hard_block<S, I>(&self, name: S, items: I) -> DocBuilder<'a, Arena<'a>>
    where
        S: Into<String>,
        I: IntoIterator,
        I::Item: Pretty<'a, Arena<'a>, ()>,
    {
        let head = self.text("(").append(name.into());
        let body = self.hardline().append(self.intersperse(items, self.hardline())).nest(4).group();
        head.append(body).append(")")
    }
    pub fn inline_block<I>(&self, name: &'static str, items: I, separator: &'static str) -> DocBuilder<'a, Arena<'a>>
    where
        I: IntoIterator,
        I::Item: Pretty<'a, Arena<'a>, ()>,
    {
        let head = self.text("(").append(name).append(self.space());
        let body = self.intersperse(items, self.text(separator)).group();
        head.append(body).append(")")
    }
}

trait VLanguage {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>>;
}

impl VLanguage for ASTKind {
    fn v_format<'a, 'b>(&'a self, arena: &'b PrettyFormatter<'b>) -> DocBuilder<'b, Arena<'b>> {
        match self {
            ASTKind::Nothing => arena.text("<<unreachable Nothing>>"),
            ASTKind::Program(v) => arena.intersperse(v.iter().map(|item| item.v_format(arena)), arena.hardline()).group(),
            ASTKind::Suite(v) => {
                let items = v.iter().map(|item| item.v_format(arena));
                arena.hard_block("block scoped", items)
            }
            ASTKind::Sequence(v) => {
                let items = v.iter().map(|item| item.v_format(arena));
                arena.intersperse(items, arena.line()).group()
            }
            ASTKind::LetBind(_) => {
                unimplemented!()
            }
            ASTKind::LambdaFunction(_) => {
                unimplemented!()
            }
            ASTKind::IfStatement(_) => {
                unimplemented!()
            }
            ASTKind::LoopStatement(_) => {
                unimplemented!()
            }
            ASTKind::InfixExpression(v) => v.v_format(arena),
            ASTKind::ApplyExpression(v) => v.v_format(arena),
            ASTKind::TupleExpression(v) => {
                if v.is_empty() {
                    return arena.text("(tuple-literal nothing)");
                }
                arena.hard_block("tuple-literal", v.iter().map(|item| item.v_format(arena)))
            }
            ASTKind::TableExpression(v) => v.v_format(arena),
            ASTKind::PairExpression(v) => v.v_format(arena),
            ASTKind::Boolean(v) => arena.as_string(v),
            ASTKind::Byte(v) => v.v_format(arena),
            ASTKind::Integer(v) => v.v_format(arena),
            ASTKind::Decimal(v) => v.v_format(arena),
            ASTKind::String(v) => v.v_format(arena),
            ASTKind::StringTemplate(v) => arena.hard_block("template-string", v.iter().map(|item| item.v_format(arena))),
            ASTKind::XMLTemplate(v) => arena.hard_block("template-xml", v.iter().map(|item| item.v_format(arena))),
            ASTKind::Symbol(v) => v.v_format(arena),
        }
    }
}
