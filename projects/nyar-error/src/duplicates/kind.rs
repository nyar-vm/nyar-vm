use super::*;

impl Debug for DuplicateKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Type => f.write_str("Type"),
            Self::Function => f.write_str("Function"),
            Self::Variable => f.write_str("Variable"),
            Self::Key => f.write_str("Key"),
        }
    }
}

impl Display for DuplicateKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Type => f.write_str("type"),
            Self::Function => f.write_str("function"),
            Self::Variable => f.write_str("variable"),
            Self::Key => f.write_str("key"),
        }
    }
}
