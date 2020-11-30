use super::*;
use pretty::{Doc, RcDoc};
use std::mem::transmute;
use Value::*;

pub const MAX_LENGTH_OF_LINE: usize = 144;

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
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
            Integer32(v) => RcDoc::text(v.to_string()).append(RcDoc::text("_i32")),
            Integer64(v) => RcDoc::text(v.to_string()).append(RcDoc::text("_i64")),
            Integer128(v) => RcDoc::text(v.to_string()).append(RcDoc::text("_i128")),

            Decimal(v) => RcDoc::as_string(v),
            Decimal32(a) => {
                let v = unsafe { transmute::<[u8; 4], f32>(*a) };
                RcDoc::text(v.to_string()).append(RcDoc::text("_f32"))
            }
            Decimal64(a) => {
                let v = unsafe { transmute::<[u8; 8], f64>(*a) };
                RcDoc::text(v.to_string()).append(RcDoc::text("_f64"))
            }

            List(xs) => RcDoc::text("[")
                .append(RcDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), RcDoc::text(", ")).nest(1).group())
                .append(RcDoc::text("]")),
            _ => unimplemented!("{:?}", self),
        }
    }
}
