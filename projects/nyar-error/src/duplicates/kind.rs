use super::*;

impl Debug for DuplicateKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DuplicateKind::Type => f.write_str("Type"),
            DuplicateKind::Function => f.write_str("Function"),
            DuplicateKind::Variable => f.write_str("Variable"),
        }
    }
}

impl Display for DuplicateKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DuplicateKind::Type => f.write_str("type"),
            DuplicateKind::Function => f.write_str("function"),
            DuplicateKind::Variable => f.write_str("variable"),
        }
    }
}
