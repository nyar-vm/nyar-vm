use super::*;
use pretty::{Doc, RcDoc};
use std::mem::transmute;
use Value::*;

pub const MAX_LENGTH_OF_LINE: usize = 144;

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        self.to_doc().render_fmt(MAX_LENGTH_OF_LINE, f)
    }
}

impl Value {
    /// Return a pretty printed format of self.
    pub fn to_doc(&self) -> RcDoc<()> {
        match self {
            Suite(xs) => RcDoc::text("")
                .append(RcDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), Doc::line()).nest(4).group())
                .append(RcDoc::text("")),

            Null => RcDoc::as_string("null"),
            Boolean(v) => RcDoc::as_string(v),

            Integer(v) => RcDoc::as_string(v),

            Decimal(v) => RcDoc::as_string(v),

            List(xs) => RcDoc::text("[")
                .append(RcDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), RcDoc::text(", ")).nest(1).group())
                .append(RcDoc::text("]")),
            _ => unimplemented!("{:?}", self),
        }
    }
}

unsafe impl GcSafe for Value {}

unsafe impl GcDrop for Value {}

unsafe impl Scan for Value {
    fn scan(&self, scanner: &mut Scanner<'_>) {
        match self {
            Null => {}
            Boolean(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Integer(_) => {}
            Decimal(_) => {}
            Character(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            String(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            List(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Suite(_) => {}
            Object(_) => {}
            Function(_) => {}
            _ => {}
        }
    }
}
