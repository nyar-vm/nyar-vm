//! Java 到 Nyar 字节码的翻译器

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
    ) -> Result<(), NyarError> {
        let mut builder = IntentBuilder::new(egraph);
        self.translate_root(ast, &mut builder)
    }

    pub fn translate_to_tree(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        self.translate_to_graph(ast, &mut egraph)?;

        let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL);
        if let Some(root_id) = egraph.classes.keys().next() {
            Ok(extractor.extract(*root_id))
        } else {
            Err(NyarError::Compile("No code generated".to_string()))
        }
    }

    fn translate_root(
        &self,
        root: &JavaRoot,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<(), NyarError> {
        for item in &root.items {
            match item {
                Item::Class(class) => self.translate_class(class, builder)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn translate_class(
        &self,
        class: &ClassDeclaration,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<(), NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            if let Member::Method(method) = member {
                let id = self.translate_method(method, builder)?;
                members.push(id);
            }
        }
        let name_id = builder.string_const(&class.name);
        let members_id = builder.seq(members);
        builder.extension("class", vec![name_id, members_id]);
        Ok(())
    }

    fn translate_method(
        &self,
        method: &MethodDeclaration,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let body_id = self.translate_block(&method.body, builder)?;
        let name_id = builder.string_const(&method.name);
        let ret_id = builder.string_const(&method.return_type);
        Ok(builder.extension("method", vec![name_id, ret_id, body_id]))
    }

    fn translate_block(
        &self,
        stmts: &[Statement],
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ids = Vec::new();
        for stmt in stmts {
            ids.push(self.translate_stmt(stmt, builder)?);
        }
        Ok(builder.seq(ids))
    }

    fn translate_stmt(
        &self,
        stmt: &Statement,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        match stmt {
            Statement::Expression(expr) => self.translate_expr(expr, builder),
            Statement::Return(Some(expr)) => {
                let val = self.translate_expr(expr, builder)?;
                Ok(builder.extension("return", vec![val]))
            }
            Statement::Return(None) => Ok(builder.extension("return", vec![])),
            Statement::Block(stmts) => self.translate_block(stmts, builder),
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(builder.constant(*v)),
            Expression::Literal(Literal::String(s)) => Ok(builder.string_const(s)),
            Expression::Literal(Literal::Boolean(b)) => Ok(builder.bool_const(*b)),
            Expression::Identifier(s) => Ok(builder.symbol(s)),
            Expression::FieldAccess(fa) => {
                let target = self.translate_expr(&fa.target, builder)?;
                let name = builder.symbol(&fa.name);
                Ok(builder.extension("get_field", vec![target, name]))
            }
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.translate_expr(arg, builder)?);
                }
                let name_id = builder.symbol(&call.name);
                let args_id = builder.seq(arg_ids);
                if let Some(target) = &call.target {
                    let target_id = self.translate_expr(target, builder)?;
                    Ok(builder.extension("call", vec![target_id, name_id, args_id]))
                } else {
                    Ok(builder.extension("call", vec![name_id, args_id]))
                }
            }
        }
    }
}
