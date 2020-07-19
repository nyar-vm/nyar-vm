use super::*;

pub struct UnionCollector {
    null_collect: bool,
    bool_collect: BooleanCollector,
    dis_join: Vec<TypingExpression>,
}

struct BooleanCollector {
    t: bool,
    f: bool,
}

impl Default for UnionCollector {
    fn default() -> Self {
        Self { null_collect: false, bool_collect: Default::default(), dis_join: vec![] }
    }
}

impl Default for BooleanCollector {
    fn default() -> Self {
        Self { t: false, f: false }
    }
}

impl AddAssign<TypingExpression> for UnionCollector {
    fn add_assign(&mut self, rhs: &TypingExpression) {
        /// rhs already refined
        match rhs {
            TypingExpression::Null => self.null_collect = true,
            TypingExpression::Boolean => {}
            TypingExpression::Literal(_) => {}
            TypingExpression::Union(_) => {}
            TypingExpression::Tuple(_) => {}
        }
    }
}

impl AddAssign<TypingExpression> for BooleanCollector {
    fn add_assign(&mut self, rhs: TypingExpression) {
        match rhs {
            TypingExpression::Boolean => {}
            TypingExpression::Literal(_) => {}
            _ => (),
        }
    }
}

impl From<UnionCollector> for TypingExpression {
    fn from(b: UnionCollector) -> Self {
        unimplemented!()
    }
}
