use crate::{engine::NyarEngine, ASTKind, ASTNode, Value, Result};

pub trait Evaluate {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Result<Value>;
}

impl Evaluate for ASTNode {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Result<Value> {
        self.kind.evaluate(ctx)
    }
}

impl Evaluate for ASTKind {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Result<Value> {
        match self {
            Self::Program(v) | Self::Suite(v) => {
                for i in v {
                    i.kind.evaluate(ctx)?;
                }
                unimplemented!()
            }
            Self::Expression { base, eos } => {
                let out = base.kind.evaluate(ctx)?;
                match *eos {
                    true => Ok(Value::Null),
                    false => { Ok(out) }
                }
            }
            Self::NumberLiteral(n) => {

            }
            _ => unimplemented!("Self::{:?}", self),
        }
    }
}
