//! Java 到 Nyar 字节码的翻译器

use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::NyarError;
use oak_java::ast::*;

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_to_graph(
        &self,
        ast: &JavaRoot,
        egraph: &mut EGraph<IKun, ConstraintAnalysis>,
    ) -> Result<Option<chomsky_uir::egraph::Id>, NyarError> {
        let mut builder = IntentBuilder::new(egraph);
        self.translate_root(ast, &mut builder)
    }

    pub fn translate_to_tree(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        if let Some(root_id) = root_id {
            let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
            Ok(extractor.extract(root_id))
        } else {
            Err(NyarError::Compile("No code generated".to_string()))
        }
    }

    fn translate_root(
        &self,
        root: &JavaRoot,
        builder: &mut IntentBuilder<'_, ConstraintAnalysis>,
    ) -> Result<Option<chomsky_uir::egraph::Id>, NyarError> {
        let mut last_id = None;
        for item in &root.items {
            match item {
                Item::Class(class) => {
                    last_id = Some(self.translate_class(class, builder)?);
                }
                _ => {}
            }
        }
        Ok(last_id)
    }

    fn translate_class(
        &self,
        class: &ClassDeclaration,
        builder: &mut IntentBuilder<'_, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            if let Member::Method(method) = member {
                let id = self.translate_method(method, builder)?;
                members.push(id);
            }
        }
        let name_id = builder.string(&class.name, Loc::default());
        let members_id = builder.seq(members, Loc::default());
        Ok(builder.extension("class", vec![name_id, members_id], Loc::default()))
    }

    fn translate_method(
        &self,
        method: &MethodDeclaration,
        builder: &mut IntentBuilder<'_, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let body_id = self.translate_block(&method.body, builder)?;
        let name_id = builder.string(&method.name, Loc::default());
        let ret_id = builder.string(&method.return_type, Loc::default());
        Ok(builder.extension("method", vec![name_id, ret_id, body_id], Loc::default()))
    }

    fn translate_block(
        &self,
        stmts: &[Statement],
        builder: &mut IntentBuilder<'_, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ids = Vec::new();
        for stmt in stmts {
            ids.push(self.translate_stmt(stmt, builder)?);
        }
        Ok(builder.seq(ids, Loc::default()))
    }

    fn translate_stmt(
        &self,
        stmt: &Statement,
        builder: &mut IntentBuilder<'_, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        match stmt {
            Statement::Expression(expr) => self.translate_expr(expr, builder),
            Statement::Return(Some(expr)) => {
                let val = self.translate_expr(expr, builder)?;
                Ok(builder.extension("return", vec![val], Loc::default()))
            }
            Statement::Return(None) => Ok(builder.extension("return", vec![], Loc::default())),
            Statement::Block(stmts) => self.translate_block(stmts, builder),
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        builder: &mut IntentBuilder<'_, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(builder.constant(*v, Loc::default())),
            Expression::Literal(Literal::String(s)) => Ok(builder.string(s, Loc::default())),
            Expression::Literal(Literal::Boolean(b)) => Ok(builder.bool(*b, Loc::default())),
            Expression::Identifier(s) => Ok(builder.symbol(s, Loc::default())),
            Expression::FieldAccess(fa) => {
                let target = self.translate_expr(&fa.target, builder)?;
                let name = builder.symbol(&fa.name, Loc::default());
                Ok(builder.extension("get_field", vec![target, name], Loc::default()))
            }
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.translate_expr(arg, builder)?);
                }
                let name_id = builder.symbol(&call.name, Loc::default());
                let args_id = builder.seq(arg_ids, Loc::default());
                if let Some(target) = &call.target {
                    let target_id = self.translate_expr(target, builder)?;
                    Ok(builder.extension("call", vec![target_id, name_id, args_id], Loc::default()))
                } else {
                    Ok(builder.extension("call", vec![name_id, args_id], Loc::default()))
                }
            }
        }
    }
}
