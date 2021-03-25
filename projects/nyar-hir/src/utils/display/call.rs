use super::*;
use crate::ast::SliceTerm;

impl Display for SliceTerm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SliceTerm::Index { index } => write!(f, "[{}]", index),
            SliceTerm::Slice { start, end, steps } => {
                write!(f, "[{}:{}:{}]", start, end, steps)
            }
        }
    }
}
