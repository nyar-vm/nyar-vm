use crate::{engine::NyarEngine, ASTKind, ASTNode, Value};

pub trait Evaluate {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Value;
}

impl Evaluate for ASTNode {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Value {
        self.kind.evaluate(ctx)
    }
}

impl Evaluate for ASTKind {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Value {
        match self {
            _ => unimplemented!("{:?}", self),
        }
    }
}
