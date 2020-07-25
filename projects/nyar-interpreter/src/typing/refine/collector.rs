use super::*;

pub struct UnionCollector {
    null_collect: bool,
    bool_collect: (bool, bool),
    dis_join: Vec<TypingExpression>,
}

struct BooleanCollector {
    t: bool,
    f: bool,
}

impl Default for UnionCollector {
    fn default() -> Self {
        Self { null_collect: false, bool_collect: (false, false), dis_join: vec![] }
    }
}

impl Default for BooleanCollector {
    fn default() -> Self {
        Self { t: false, f: false }
    }
}

impl AddAssign<&TypingExpression> for UnionCollector {
    fn add_assign(&mut self, rhs: &TypingExpression) {
        match rhs {
            TypingExpression::Null => self.null_collect = true,
            TypingExpression::Boolean => self.bool_collect = (true, true),
            TypingExpression::Literal(t) => match t.as_ref() {
                Value::Null => self.null_collect = true,
                Value::Boolean(true) => self.bool_collect.0 = true,
                Value::Boolean(false) => self.bool_collect.1 = true,
                _ => self.dis_join.push(rhs.to_owned()),
            },
            TypingExpression::Union(terms) => {
                for term in terms {
                    *self += term
                }
            }
            TypingExpression::Tuple(_) => self.dis_join.push(rhs.to_owned()),
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
        let mut terms = vec![];
        if b.null_collect {
            terms.push(TypingExpression::Null)
        }
        match (b.bool_collect.0, b.bool_collect.1) {
            (true, true) => terms.push(TypingExpression::Boolean),
            (true, false) => terms.push(TypingExpression::boolean(true)),
            (false, true) => terms.push(TypingExpression::boolean(false)),
            (false, false) => (),
        }
        terms.extend(b.dis_join);
        TypingExpression::Union(terms)
    }
}
