use std::fmt::{Debug, Display, Formatter, Write};

use pretty::{Arena, DocAllocator, DocBuilder};

use nyar_error::Result;

use crate::{
    ast::{ByteLiteral, DecimalLiteral, IntegerLiteral, Symbol},
    ASTKind, ASTNode,
};

mod atom;
mod call;

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::Nothing, span: Default::default() }
    }
}

impl From<ASTKind> for ASTNode {
    fn from(kind: ASTKind) -> Self {
        Self { kind, span: Default::default() }
    }
}

impl ASTNode {
    pub fn pretty_print(&self, width: usize) -> Result<String> {
        self.kind.pretty_print(width)
    }
}

impl ASTKind {
    pub fn pretty_print(&self, width: usize) -> Result<String> {
        let mut out = String::new();
        let arena = Arena::new();
        let doc = self.v_format(&arena);
        doc.render_fmt(width, &mut out)?;
        Ok(out)
    }
}

trait VLanguage {
    fn v_format<'a, 'b>(&'a self, arena: &'b Arena<'b>) -> DocBuilder<'b, Arena<'b>>;

    fn hard_head<'a, 'b>(&self, name: &'static str, arena: &'b Arena<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.text("(").append(name).append(arena.hardline())
    }

    fn soft_head<'a, 'b>(name: &'static str, arena: &'b Arena<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.text("(").append(name).append(arena.softline())
    }

    fn inline_head<'a, 'b>(name: &'static str, arena: &'b Arena<'b>) -> DocBuilder<'b, Arena<'b>> {
        arena.text("(").append(name).append(arena.line())
    }
}

impl VLanguage for ASTKind {
    fn v_format<'a, 'b>(&'a self, arena: &'b Arena<'b>) -> DocBuilder<'b, Arena<'b>> {
        match self {
            ASTKind::Nothing => arena.text("<<unreachable Nothing>>"),
            ASTKind::Program(v) => {
                let items = v.iter().map(|item| item.v_format(arena));
                arena.intersperse(items, arena.hardline()).group()
            }
            ASTKind::Suite(v) => {
                let items = v.iter().map(|item| item.v_format(arena));
                let head = self.hard_head("scoped-block", arena);
                let body = arena.intersperse(items, arena.line()).nest(1).group();
                head.append(body).append(arena.text(")"))
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
            ASTKind::InfixExpression(_) => {
                unimplemented!()
            }
            ASTKind::ApplyExpression(v) => v.v_format(arena),
            ASTKind::TupleExpression(_) => {
                unimplemented!()
            }
            ASTKind::TableExpression(_) => {
                unimplemented!()
            }
            ASTKind::PairExpression(_) => {
                unimplemented!()
            }
            ASTKind::Boolean(_) => {
                unimplemented!()
            }
            ASTKind::Byte(_) => {
                unimplemented!()
            }
            ASTKind::Integer(_) => {
                unimplemented!()
            }
            ASTKind::Decimal(_) => {
                unimplemented!()
            }
            ASTKind::String(_) => {
                unimplemented!()
            }
            ASTKind::StringTemplate(_) => {
                unimplemented!()
            }
            ASTKind::XMLTemplate(_) => {
                unimplemented!()
            }
            ASTKind::Symbol(v) => v.v_format(arena),
        }
    }
}

pub fn write_tuple(name: &str, v: &[impl Debug], f: &mut Formatter<'_>) -> std::fmt::Result {
    if v.is_empty() {
        f.write_str(name)?;
        return f.write_str("()");
    }
    let mut w = &mut f.debug_tuple(name);
    for node in v {
        w = w.field(node)
    }
    w.finish()
}
