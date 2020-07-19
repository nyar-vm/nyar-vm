mod collector;

use self::collector::UnionCollector;
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
                _ => {
                    let mut collector = UnionCollector::default();
                    for i in u {
                        i.refine();
                        collector += &*i;
                    }
                    *self = Self::from(collector)
                }
            },
            Self::Tuple(_) => (),
        }
    }
}

impl EffectExpression {
    pub fn refine(&mut self) {}
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
