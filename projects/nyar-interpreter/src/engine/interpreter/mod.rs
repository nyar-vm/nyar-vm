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
                let mut out = vec![];
                for i in v {
                    let o = i.kind.evaluate(ctx)?;
                    match o {
                        Value::Null => {},
                        _ => out.push(o)
                    }
                }
                return Ok(Value::Suite(out))
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
        match (self.is_integer, &self.handler) {
            (true, Some(s)) => ctx.root_module.get_integer_handlers().parse_integer(s, &self.value),
            (true, None) => {
                let s = &ctx.root_module.get_integer_handler();
                ctx.root_module.get_integer_handlers().parse_integer(s, &self.value)
            }
            (false, Some(s)) => ctx.root_module.get_decimal_handlers().parse_decimal(s, &self.value),
            (false, None) => {
                let s = &ctx.root_module.get_decimal_handler();
                ctx.root_module.get_decimal_handlers().parse_decimal(s, &self.value)
            }
        }
    }
}
