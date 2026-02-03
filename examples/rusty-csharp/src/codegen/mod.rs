//! CSharp 到 Nyar 字节码的翻译器

use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::NyarError;
use oak_java::ast::*;

/// CSharp 翻译器上下文
///
/// 用于管理翻译过程中的状态，如符号表、E-Graph 构建器等。
pub struct TranslatorContext<'a> {
    pub builder: IntentBuilder<'a, ConstraintAnalysis>,
}

impl<'a> TranslatorContext<'a> {
    pub fn new(egraph: &'a mut EGraph<IKun, ConstraintAnalysis>) -> Self {
        Self {
            builder: IntentBuilder::new(egraph),
        }
    }

    /// 解析内置函数映射
    pub fn resolve_builtin(&mut self, target: &str, method: &str, args: Vec<chomsky_uir::egraph::Id>, loc: Loc) -> Option<chomsky_uir::egraph::Id> {
        match (target, method) {
            ("System.Console", "WriteLine") | ("Console", "WriteLine") => {
                Some(self.builder.cross_lang_call("nyar", "std::io::println", args, loc))
            }
            ("System.Console", "Write") | ("Console", "Write") => {
                Some(self.builder.cross_lang_call("nyar", "std::io::print", args, loc))
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

    pub fn translate_to_graph(
        &self,
        ast: &JavaRoot,
        egraph: &mut EGraph<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ctx = TranslatorContext::new(egraph);
        self.translate_root(ast, &mut ctx)
    }

    pub fn translate_to_tree(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        let extractor =
            chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DefaultCostModel::default());

        Ok(extractor.extract(root_id))
    }

    fn translate_root(
        &self,
        root: &JavaRoot,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut classes = Vec::new();
        for item in &root.items {
            if let Item::Class(class) = item {
                classes.push(self.translate_class(class, ctx)?);
            }
        }
        Ok(ctx.builder.module("root", classes))
    }

    fn translate_class(
        &self,
        class: &ClassDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            if let Member::Method(method) = member {
                let id = self.translate_method(method, ctx)?;
                members.push(id);
            }
        }
        Ok(ctx.builder.module(&class.name, members))
    }

    fn translate_method(
        &self,
        method: &MethodDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let body_id = self.translate_block(&method.body, ctx)?;
        
        // 目前简单处理，将方法作为导出函数
        // TODO: 处理参数
        let params = Vec::new();
        let lambda = ctx.builder.lambda(params, body_id, loc);
        Ok(ctx.builder.export(&method.name, lambda, loc))
    }

    fn translate_block(
        &self,
        statements: &[Statement],
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut stmts = Vec::new();
        for stmt in statements {
            stmts.push(self.translate_stmt(stmt, ctx)?);
        }
        Ok(ctx.builder.block(stmts, Loc::unknown()))
    }

    fn translate_stmt(
        &self,
        stmt: &Statement,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match stmt {
            Statement::Expression(expr) => self.translate_expr(expr, ctx),
            Statement::Return(Some(expr)) => {
                let val = self.translate_expr(expr, ctx)?;
                Ok(ctx.builder.return_(val, loc))
            }
            Statement::Return(None) => {
                let void = ctx.builder.constant(0, loc); // Placeholder for void
                Ok(ctx.builder.return_(void, loc))
            },
            Statement::Block(inner) => self.translate_block(inner, ctx),
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(ctx.builder.constant(*v, loc)),
            Expression::Literal(Literal::String(s)) => Ok(ctx.builder.string(s, loc)),
            Expression::Identifier(s) => {
                // 检查是否是内置函数全称
                if let Some(builtin) = ctx.resolve_builtin(s, "", vec![], loc) {
                    return Ok(builtin);
                }
                Ok(ctx.builder.symbol(s, loc))
            }
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.translate_expr(arg, ctx)?);
                }

                // 处理内置函数
                if let Some(target) = &call.target {
                    let target_name = self.expr_to_string(target);
                    if let Some(builtin) = ctx.resolve_builtin(&target_name, &call.name, arg_ids.clone(), loc) {
                        return Ok(builtin);
                    }
                } else {
                    if let Some(builtin) = ctx.resolve_builtin("", &call.name, arg_ids.clone(), loc) {
                        return Ok(builtin);
                    }
                }

                let name_id = ctx.builder.symbol(&call.name, loc);
                if let Some(target) = &call.target {
                    let target_id = self.translate_expr(target, ctx)?;
                    let args_id = ctx.builder.seq(arg_ids, loc);
                    Ok(ctx.builder.extension("call", vec![target_id, name_id, args_id], loc))
                } else {
                    Ok(ctx.builder.call(name_id, arg_ids, loc))
                }
            }
            Expression::FieldAccess(access) => {
                let target_id = self.translate_expr(&access.target, ctx)?;
                let name_id = ctx.builder.symbol(&access.name, loc);
                Ok(ctx.builder.extension("field", vec![target_id, name_id], loc))
            }
            _ => Ok(ctx.builder.constant(0, loc)),
        }
    }

    fn expr_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(s) => s.clone(),
            Expression::FieldAccess(fa) => {
                let target = self.expr_to_string(&fa.target);
                format!("{}.{}", target, fa.name)
            }
            _ => "".to_string(),
        }
    }
}
