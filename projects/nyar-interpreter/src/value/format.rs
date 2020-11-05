use super::*;

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Boolean(v) => write!(f, "{}", v),
            Value::Integer(v) => write!(f, "{}", v),
            Value::Decimal(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "{}", v),
            Value::List(v) => write!(f, "{:?}", v),
            Value::Object(v) => write!(f, "{:?}", v),
            Value::Function(v) => write!(f, "{:?}", v),
            // Value::CustomClass(_) => write!(f, "Class"),
            _ => unimplemented!("{}", self),
        }
    }
}
