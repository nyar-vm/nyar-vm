use crate::{engine::NyarEngine, ASTKind, ASTNode, Value, Result};
use nyar_hir::ast::NumberLiteral;
use crate::engine::module::DefaultIntegerHandler;

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
                n.evaluate(ctx)
            }
            _ => unimplemented!("Self::{:?}", self),
        }
    }
}


impl Evaluate for NumberLiteral {
    fn evaluate(&self, ctx: &mut NyarEngine) -> Result<Value> {
        match self.is_integer {
            true => {
                get_integer_handler(self, ctx).parse(&self.value)
            }
            false => {
                unimplemented!()
            }
        }
    }
}

fn get_integer_handler(num: &NumberLiteral, ctx: &NyarEngine) -> DefaultIntegerHandler {
    match &num.handler {
        Some(s) => {
            match s.as_str() {
                "i8" => {
                    DefaultIntegerHandler::I8
                }
                "int" => {
                    DefaultIntegerHandler::IBig
                }
                "uint" => {
                    DefaultIntegerHandler::UBig
                }
                _ => unimplemented!()
            }
        },
        None => {
            ctx.root_module.get_integer_handler()
        }
    }

}