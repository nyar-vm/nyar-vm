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
            Integer32(v) => RcDoc::text(v.to_string()).append(RcDoc::text("_i32")),
            Integer64(v) => RcDoc::text(v.to_string()).append(RcDoc::text("_i64")),
            Integer128(v) => RcDoc::text(v.to_string()).append(RcDoc::text("_i128")),

            Decimal(v) => RcDoc::as_string(v),
            Decimal32(a) => RcDoc::text(a.unwrap().to_string()).append(RcDoc::text("_f32")),
            Decimal64(a) => RcDoc::text(a.unwrap().to_string()).append(RcDoc::text("_f64")),

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
            // Character(v) => {
            //     scanner.scan(v);
            //     check_gc_drop(v);
            // }
            Integer8(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Integer16(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Integer32(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Integer64(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Integer128(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            IntegerSized(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            UnsignedInteger8(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            UnsignedInteger16(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            UnsignedInteger32(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            UnsignedInteger64(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            UnsignedInteger128(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            UnsignedIntegerSized(v) => {
                scanner.scan(v);
                check_gc_drop(v);
            }
            Decimal32(_) => {}
            Decimal64(_) => {}
            Integer(_) => {}
            UnsignedInteger(_) => {}
            Decimal(_) => {}
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
