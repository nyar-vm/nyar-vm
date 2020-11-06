use super::*;
use Value::*;
use std::mem::transmute;
use pretty::RcDoc;

pub const LIST_MAX_DISPLAY:usize = 255;

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {

        match self {
            Suite(v) => {
                for i in v {
                    writeln!(f, "{}", i)?;
                }
                Ok(())
            },
            Null => write!(f, "null"),
            Boolean(v) => write!(f, "{}", v),

            Integer(v) => write!(f, "{}", v),
            Integer32(v) => write!(f, "{}_i32", v),
            Integer64(v) => write!(f, "{}_i64", v),
            Integer128(v) => write!(f, "{}_i128", v),

            Decimal(v) => write!(f, "{}", v),
            Decimal32(a) => {
                let v =  unsafe {
                    transmute::<[u8;4], f32>(*a)
                };
                write!(f, "{}", v)
            }
            Decimal64(a) => {
                let v =  unsafe {
                    transmute::<[u8;8], f64>(*a)
                };
                write!(f, "{}", v)
            }

            List(v) => {
                match v.len() {
                    0 => write!(f, "[]"),
                    n if n >= LIST_MAX_DISPLAY => {
                        write!(f, "[, ...[({}) terms]]", v.len() - LIST_MAX_DISPLAY)
                    }
                    // n ∈ [1, 255]
                    _ => {
                        write!(f, "{:#?}", v)
                    }
                }
            }

            // Value::String(v) => write!(f, "{}", v),
            // Value::List(v) => write!(f, "{:?}", v),
            // Value::Object(v) => write!(f, "{:?}", v),
            // Value::Function(v) => write!(f, "{:?}", v),
            // Value::CustomClass(_) => write!(f, "Class"),
            _ => unimplemented!("{:?}", self),
        }
    }
}


impl Value {
    /// Return a pretty printed format of self.
    pub fn to_doc(&self) -> RcDoc<()> {
        match *self {
            Atom(ref x) => RcDoc::as_string(x),
            List(ref xs) =>
                RcDoc::text("(")
                    .append(RcDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), Doc::line()).nest(1).group())
                    .append(RcDoc::text(")"))
        }
    }
}
