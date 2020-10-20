use super::*;

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.debug_tuple("Null").finish(),
            Value::Boolean(v) => f.debug_tuple("Boolean").field(v).finish(),
            Value::Integer(v) => f.debug_tuple("Integer").field(v).finish(),
            Value::Decimal(v) => f.debug_tuple("Decimal").field(v).finish(),
            Value::String(v) => f.debug_tuple("String").field(v).finish(),
            Value::List(v) => f.debug_tuple("List").field(v).finish(),
            Value::Object(v) => f.debug_tuple("Object").field(v).finish(),
            Value::Function(v) => f.debug_tuple("Function").field(v).finish(),
            Value::CustomClass(_) => f.debug_tuple("Class").finish(),
            _ => unimplemented!("{}", self),
        }
    }
}

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
            Value::CustomClass(_) => write!(f, "Class"),
            _ => unimplemented!("{}", self),
        }
    }
}
