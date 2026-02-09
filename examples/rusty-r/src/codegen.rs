//! R 代码生成实现

use chomsky_uir::{EGraph, IKun, IntentBuilder};
use nyar_types::{Loc, NyarError};
use oak_r::ast::*;
use oak_r::kind::RSyntaxKind;

/// R 翻译器上下文
pub struct TranslatorContext<'a, A: chomsky_uir::Analysis<IKun> = ()> {
    pub builder: IntentBuilder<'a, A>,
}

impl<'a, A: chomsky_uir::Analysis<IKun>> TranslatorContext<'a, A> {
    pub fn new(egraph: &'a mut EGraph<IKun, A>) -> Self {
        Self {
            builder: IntentBuilder::new(egraph),
        }
    }

    pub fn new_with_builder(builder: IntentBuilder<'a, A>) -> Self {
        Self { builder }
    }

    /// 解析内置函数映射
    pub fn resolve_builtin(&mut self, method: &str, args: Vec<chomsky_uir::egraph::Id>, loc: Loc) -> Option<chomsky_uir::egraph::Id> {
        match method {
            "print" | "cat" => {
                Some(self.builder.cross_lang_call("nyar", "std::io", "println", args, loc))
            }
            "sum" => {
                Some(self.builder.cross_lang_call("nyar", "std::math", "sum", args, loc))
            }
            "mean" => {
                Some(self.builder.cross_lang_call("nyar", "std::math", "mean", args, loc))
            }
            _ => None,
        }
    }
}

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_root<A: chomsky_uir::Analysis<IKun>>(
        &self,
        root: &RRoot,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for stmt in &root.statements {
            items.push(self.translate_statement(stmt, ctx)?);
        }
        Ok(ctx.builder.module("main", items))
    }

    fn translate_statement<A: chomsky_uir::Analysis<IKun>>(
        &self,
        stmt: &Statement,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match stmt {
            Statement::Assignment { name, expr, .. } => {
                let val_id = self.translate_expr(expr, ctx)?;
                let sym_id = ctx.builder.symbol(&name.name, loc);
                Ok(ctx.builder.state_update(sym_id, val_id))
            }
            Statement::ExprStmt { expr, .. } => self.translate_expr(expr, ctx),
            Statement::FunctionDef { name, params, body, .. } => {
                let mut body_items = Vec::new();
                for s in body {
                    body_items.push(self.translate_statement(s, ctx)?);
                }
                let body_id = ctx.builder.seq(body_items);
                let param_names = params.iter().map(|p| p.name.clone()).collect();
                let lambda_id = ctx.builder.lambda(param_names, body_id);
                Ok(ctx.builder.export(&name.name, lambda_id))
            }
        }
    }

    fn translate_expr<A: chomsky_uir::Analysis<IKun>>(
        &self,
        expr: &Expr,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match expr {
            Expr::Ident(id) => Ok(ctx.builder.symbol(&id.name, loc)),
            Expr::Literal { value, .. } => {
                // 尝试解析为整数或浮点数
                if let Ok(i) = value.parse::<i64>() {
                    Ok(ctx.builder.constant(i))
                } else if let Ok(f) = value.parse::<f64>() {
                    Ok(ctx.builder.float_constant(f.to_bits()))
                } else {
                    Ok(ctx.builder.string_constant(value))
                }
            }
            Expr::Bool { value, .. } => Ok(ctx.builder.boolean_constant(*value)),
            Expr::Null { .. } => Ok(ctx.builder.symbol("NULL", loc)),
            Expr::Call { callee, args, .. } => {
                let mut arg_ids = Vec::new();
                for arg in args {
                    arg_ids.push(self.translate_expr(arg, ctx)?);
                }
                
                if let Expr::Ident(id) = &**callee {
                    if let Some(builtin) = ctx.resolve_builtin(&id.name, arg_ids.clone(), loc) {
                        return Ok(builtin);
                    }
                }
                
                let callee_id = self.translate_expr(callee, ctx)?;
                Ok(ctx.builder.apply(callee_id, arg_ids))
            }
            Expr::Binary { left, op, right, .. } => {
                let l_id = self.translate_expr(left, ctx)?;
                let r_id = self.translate_expr(right, ctx)?;
                let op_str = match op {
                    RSyntaxKind::Plus => "+",
                    RSyntaxKind::Minus => "-",
                    RSyntaxKind::Star => "*",
                    RSyntaxKind::Slash => "/",
                    RSyntaxKind::Caret => "^",
                    RSyntaxKind::EqualEqual => "==",
                    RSyntaxKind::NotEqual => "!=",
                    RSyntaxKind::Less => "<",
                    RSyntaxKind::Greater => ">",
                    RSyntaxKind::LessEqual => "<=",
                    RSyntaxKind::GreaterEqual => ">=",
                    RSyntaxKind::And => "&",
                    RSyntaxKind::Or => "|",
                    RSyntaxKind::AndAnd => "&&",
                    RSyntaxKind::OrOr => "||",
                    _ => "unknown_op",
                };
                let op_sym = ctx.builder.symbol(op_str, loc);
                Ok(ctx.builder.apply(op_sym, vec![l_id, r_id]))
            }
            Expr::Unary { op, expr, .. } => {
                let expr_id = self.translate_expr(expr, ctx)?;
                let op_str = match op {
                    RSyntaxKind::Minus => "-",
                    RSyntaxKind::Plus => "+",
                    RSyntaxKind::Not => "!",
                    _ => "unknown_unary",
                };
                let op_sym = ctx.builder.symbol(op_str, loc);
                Ok(ctx.builder.apply(op_sym, vec![expr_id]))
            }
        }
    }
}
