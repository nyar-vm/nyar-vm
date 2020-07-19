use super::*;
use std::ops::AddAssign;

impl Typing {
    pub fn refine(&mut self) {
        if let Some(s) = &mut self.typing {
            s.refine()
        };
        if let Some(s) = &mut self.effect {
            s.refine()
        }
    }
}

impl TypingExpression {
    pub fn refine(&mut self) {
        match self {
            Self::Null => {}
            Self::Boolean => {}
            Self::Literal(v) => {
                if let Value::Null = v.as_ref() {
                    *self = Self::Null
                }
            }
            Self::Union(u) => match u.len() {
                0 => *self = Self::Tuple(vec![]),
                1 => {
                    *self = u[0].to_owned();
                    self.refine()
                }
                _ => (),
            },
            Self::Tuple(_) => (),
        }
    }
}

impl EffectExpression {
    pub fn refine(&mut self) {}
}

struct UnionCollector {
    null_collect: bool,
    bool_collect: (bool, bool),
    dis_join: Vec<TypingExpression>,
}

impl Default for UnionCollector {
    fn default() -> Self {
        Self { null_collect: false, bool_collect: (false, false), dis_join: vec![] }
    }
}

impl AddAssign<TypingExpression> for UnionCollector {
    fn add_assign(&mut self, rhs: TypingExpression) {
        match rhs {
            TypingExpression::Null => {}
            TypingExpression::Boolean => {}
            TypingExpression::Literal(_) => {}
            TypingExpression::Union(_) => {}
            TypingExpression::Tuple(_) => {}
        }
    }
}

#[test]
fn test() {
    let t = TypingExpression::Literal(box Value::True);
    let null = TypingExpression::Literal(box Value::Null);

    let union = vec![null, t];
    let mut typing = TypingExpression::Union(union);
    typing.refine();
    println!("{}", typing)
}
