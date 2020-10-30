use crate::{
    engine::{module::DefaultIntegerHandler, NyarEngine},
    ASTKind, ASTNode, Result, Value,
};
use nyar_hir::ast::NumberLiteral;

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
                    false => Ok(out),
                }
            }
            Self::NumberLiteral(n) => n.evaluate(ctx),
            _ => unimplemented!("Self::{:?}", self),
        }
    }
}

impl Evaluate for NumberLiteral {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Result<Value> {
        match self.is_integer {
            true => ctx.root_module.get_integer_handler().parse_integer(&self.handler,&self.value),
            false => ctx.root_module.get_decimal_handler().parse_decimal(&self.handler, &self.value),
        }
    }
}
