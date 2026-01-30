//! CSharp 到 Nyar 字节码的翻译器

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
        for class in &root.classes {
            self.translate_class(class, builder)?;
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
            if let ClassMember::Method(method) = member {
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
        let ret_id = builder.string_const("void");
        Ok(builder.extension("method", vec![name_id, ret_id, body_id]))
    }

    fn translate_block(
        &self,
        block: &Block,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut stmts = Vec::new();
        for stmt in &block.statements {
            stmts.push(self.translate_stmt(stmt, builder)?);
        }
        Ok(builder.seq(stmts))
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
            _ => Ok(builder.constant(0)),
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        builder: &mut IntentBuilder<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(builder.constant(*v as i64)),
            Expression::Literal(Literal::String(s)) => Ok(builder.string_const(s)),
            Expression::Identifier(s) => Ok(builder.symbol(s)),
            Expression::Binary(left, op, right) => {
                let l = self.translate_expr(left, builder)?;
                let r = self.translate_expr(right, builder)?;
                Ok(builder.extension(&format!("{:?}", op), vec![l, r]))
            }
            Expression::Call(name, args) => {
                let mut arg_ids = Vec::new();
                for arg in args {
                    arg_ids.push(self.translate_expr(arg, builder)?);
                }
                let name_id = builder.symbol(name);
                let args_id = builder.seq(arg_ids);
                Ok(builder.extension("call", vec![name_id, args_id]))
            }
        }
    }
}

