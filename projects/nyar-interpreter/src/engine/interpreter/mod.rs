use crate::{engine::NyarEngine, ASTKind, ASTNode, Result, Value};
use nyar_hir::ast::IntegerLiteral;

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
        let value = match self {
            // Self::Program(v) | Self::Suite(v) => {
            //     let mut out = vec![];
            //     for i in v {
            //         let o = i.kind.evaluate(ctx)?;
            //         out.push(o)
            //     }
            //     Value::Suite(out)
            // }
            // Self::Expression { base, eos } => {
            //     let out = base.kind.evaluate(ctx)?;
            //     match *eos {
            //         true => Value::Null,
            //         false => out,
            //     }
            // }
            Self::Integer(n) => n.evaluate(ctx)?,
            Self::Decimal(s) => s.evaluate(ctx)?,
            Self::Nothing => Value::Null,
            Self::Boolean(v) => Value::Boolean(*v),
            // Self::ListExpression(v) => {
            //     let mut out = vec![];
            //     for i in v {
            //         let o = i.kind.evaluate(ctx)?;
            //         match o {
            //             Value::Null => {}
            //             _ => out.push(o),
            //         }
            //     }
            //     Value::List(out)
            // }
            _ => unimplemented!("Self::{:?}", self),
        };
        return Ok(value);
    }
}

impl Evaluate for IntegerLiteral {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Result<Value> {
        ctx.get_integer_handlers().parse_integer(s, &self.value)
    }
}
