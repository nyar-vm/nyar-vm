//! Gaia 指令生成器 (已重构为意图树生成器)

use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder, Id};
use nyar_types::NyarError;
use oak_lua::ast::*;

/// Gaia 翻译器，将 Lua AST 转换为意图树
pub struct GaiaTranslator;

impl GaiaTranslator {
    /// 创建新的 Gaia 翻译器
    pub fn new() -> Self {
        Self
    }

    /// 从 Lua AST 生成意图树
    pub fn translate_to_tree(&self, ast: &LuaRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = IntentBuilder::new(&mut egraph);

        let mut statements = Vec::new();
        for stmt in &ast.statements {
            if let Ok(id) = self.translate_statement(stmt, &mut builder) {
                statements.push(id);
            }
        }

        let loc = chomsky_types::Loc::default();
        let root = builder.block(statements, loc);
        
        let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        Ok(extractor.extract(root))
    }

    fn translate_statement(&self, stmt: &LuaStatement, builder: &mut IntentBuilder<ConstraintAnalysis>) -> Result<Id, NyarError> {
        let loc = chomsky_types::Loc::default();
        match stmt {
            LuaStatement::Local(s) => {
                let mut ids = Vec::new();
                for (i, name) in s.names.iter().enumerate() {
                    let val = if let Some(expr) = s.values.get(i) {
                        self.translate_expression(expr, builder)?
                    } else {
                        builder.symbol("nil", loc)
                    };
                    ids.push(builder.assign(name, val, loc));
                }
                Ok(builder.block(ids, loc))
            }
            LuaStatement::Expression(expr) => self.translate_expression(expr, builder),
            LuaStatement::Return(s) => {
                let mut vals = Vec::new();
                for expr in &s.values {
                    vals.push(self.translate_expression(expr, builder)?);
                }
                if vals.len() == 1 {
                    Ok(builder.return_(vals[0], loc))
                } else {
                    let tuple = builder.extension("tuple", vals, loc);
                    Ok(builder.return_(tuple, loc))
                }
            }
            _ => {
                // TODO: 更多语句支持
                Ok(builder.symbol("nil", loc))
            }
        }
    }

    fn translate_expression(&self, expr: &LuaExpression, builder: &mut IntentBuilder<ConstraintAnalysis>) -> Result<Id, NyarError> {
        let loc = chomsky_types::Loc::default();
        match expr {
            LuaExpression::Number(n) => Ok(builder.float(*n, loc)),
            LuaExpression::String(s) => Ok(builder.string(s, loc)),
            LuaExpression::Boolean(b) => Ok(builder.bool(*b, loc)),
            LuaExpression::Nil => Ok(builder.symbol("nil", loc)),
            LuaExpression::Identifier(id) => Ok(builder.symbol(id, loc)),
            LuaExpression::Binary(bin) => {
                let left = self.translate_expression(&bin.left, builder)?;
                let right = self.translate_expression(&bin.right, builder)?;
                match bin.op.as_str() {
                    "+" => Ok(builder.add_op(left, right, loc)),
                    "-" => Ok(builder.sub_op(left, right, loc)),
                    "*" => Ok(builder.mul_op(left, right, loc)),
                    "/" => Ok(builder.div_op(left, right, loc)),
                    _ => Ok(builder.symbol("nil", loc)),
                }
            }
            LuaExpression::Call(call) => {
                let func = self.translate_expression(&call.function, builder)?;
                let mut args = Vec::new();
                for arg in &call.arguments {
                    args.push(self.translate_expression(arg, builder)?);
                }
                Ok(builder.call(func, args, loc))
            }
            _ => {
                // TODO: 更多表达式支持
                Ok(builder.symbol("nil", loc))
            }
        }
    }
}
