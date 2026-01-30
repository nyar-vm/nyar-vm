//! Kotlin 到 Nyar 字节码的翻译器

use nyar_vm::NyarError;
use chomsky_uir::{IntentBuilder, IKunTree, EGraph, IKun, ConstraintAnalysis, Id};
use chomsky_extract::IKunExtractor;
use chomsky_cost::DEFAULT_COST_MODEL;
use oak_kotlin::ast::*;
use oak_kotlin::kind::KotlinSyntaxKind;
use oak_core::{GreenNode, GreenTree, Language};
use oak_kotlin::language::KotlinLanguage;

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_to_tree(&self, root: &KotlinRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(root, &mut egraph)?;
        let extractor = IKunExtractor::new(&egraph, &DEFAULT_COST_MODEL);
        Ok(extractor.extract(root_id))
    }

    pub fn translate_to_graph(&self, root: &KotlinRoot, egraph: &mut EGraph<IKun, ConstraintAnalysis>) -> Result<Id, NyarError> {
        let mut builder = IntentBuilder::new(egraph);
        let mut members = Vec::new();
        
        for decl in &root.declarations {
            members.push(self.translate_declaration(decl, &mut builder)?);
        }
        
        let root_id = builder.seq(members);
        builder.set_root(root_id);
        Ok(root_id)
    }

    fn translate_declaration(&self, decl: &Declaration, builder: &mut IntentBuilder) -> Result<Id, NyarError> {
        match decl {
            Declaration::Class { name, members, .. } => {
                let mut class_members = Vec::new();
                for member in members {
                    class_members.push(self.translate_declaration(member, builder)?);
                }
                let members_seq = builder.seq(class_members);
                Ok(builder.extension("class", vec![
                    builder.string(name.clone()),
                    members_seq
                ]))
            }
            Declaration::Function { name, params, body, .. } => {
                let mut param_ids = Vec::new();
                for param in params {
                    param_ids.push(builder.extension("parameter", vec![
                        builder.string(param.name.clone()),
                        builder.string(param.type_name.clone().unwrap_or_else(|| "Any".to_string())),
                    ]));
                }
                let params_seq = builder.seq(param_ids);
                
                let mut stmt_ids = Vec::new();
                for stmt in body {
                    stmt_ids.push(self.translate_statement(stmt, builder)?);
                }
                let body_seq = builder.seq(stmt_ids);
                
                Ok(builder.extension("method", vec![
                    builder.string(name.clone()),
                    builder.string("void"), // TODO: proper return type
                    params_seq,
                    body_seq
                ]))
            }
            Declaration::Variable { name, is_val, .. } => {
                Ok(builder.extension("variable", vec![
                    builder.string(name.clone()),
                    builder.bool(*is_val),
                ]))
            }
        }
    }

    fn translate_statement(&self, stmt: &Statement, builder: &mut IntentBuilder) -> Result<Id, NyarError> {
        match stmt {
            Statement::Return(expr) => {
                let expr_id = if let Some(e) = expr {
                    builder.string(e.clone()) // TODO: parse expression
                } else {
                    builder.seq(vec![])
                };
                Ok(builder.extension("return", vec![expr_id]))
            }
            Statement::Expression(expr) => {
                Ok(builder.string(expr.clone())) // TODO: proper expression translation
            }
            Statement::Variable { name, is_val } => {
                Ok(builder.extension("variable", vec![
                    builder.string(name.clone()),
                    builder.bool(*is_val),
                ]))
            }
        }
    }
}

struct TranslationContext<'a> {
    builder: &'a mut IntentBuilder<'a>,
    source: &'a str,
    offset: usize,
}

impl<'a> TranslationContext<'a> {
    fn translate_node(&mut self, node: &GreenNode<'static, KotlinLanguage>) -> Result<Id, NyarError> {
        // This is no longer used, but kept for reference if needed
        Ok(self.builder.seq(vec![]))
    }
}
