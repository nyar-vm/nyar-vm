//! CSharp 到 Nyar 字节码的翻译器

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
    ) -> Result<(), NyarError> {
        let mut builder = IntentBuilder::new(egraph);
        self.translate_root(ast, &mut builder)
    }

    pub fn translate_to_tree(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        self.translate_to_graph(ast, &mut egraph)?;

        let extractor =
            chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DefaultCostModel::default());
        let root_id = egraph.classes.iter().next().map(|entry| *entry.key());
        if let Some(root_id) = root_id {
            Ok(extractor.extract(root_id))
        } else {
            Err(NyarError::Compile("No code generated".to_string()))
        }
    }

    fn translate_root(
        &self,
        root: &JavaRoot,
        builder: &mut IntentBuilder<ConstraintAnalysis>,
    ) -> Result<(), NyarError> {
        for item in &root.items {
            if let Item::Class(class) = item {
                self.translate_class(class, builder)?;
            }
        }
        Ok(())
    }

    fn translate_class(
        &self,
        class: &ClassDeclaration,
        builder: &mut IntentBuilder<ConstraintAnalysis>,
    ) -> Result<(), NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            if let Member::Method(method) = member {
                let id = self.translate_method(method, builder)?;
                members.push(id);
            }
        }
        let loc = Loc::unknown();
        let name_id = builder.string(&class.name, loc);
        let members_id = builder.seq(members, loc);
        builder.extension("class", vec![name_id, members_id], loc);
        Ok(())
    }

    fn translate_method(
        &self,
        method: &MethodDeclaration,
        builder: &mut IntentBuilder<ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let body_id = self.translate_block(&method.body, builder)?;
        let name_id = builder.string(&method.name, loc);
        let ret_id = builder.string(&method.return_type, loc);
        Ok(builder.extension("method", vec![name_id, ret_id, body_id], loc))
    }

    fn translate_block(
        &self,
        statements: &[Statement],
        builder: &mut IntentBuilder<ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut stmts = Vec::new();
        for stmt in statements {
            stmts.push(self.translate_stmt(stmt, builder)?);
        }
        Ok(builder.seq(stmts, Loc::unknown()))
    }

    fn translate_stmt(
        &self,
        stmt: &Statement,
        builder: &mut IntentBuilder<ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match stmt {
            Statement::Expression(expr) => self.translate_expr(expr, builder),
            Statement::Return(Some(expr)) => {
                let val = self.translate_expr(expr, builder)?;
                Ok(builder.extension("return", vec![val], loc))
            }
            Statement::Return(None) => Ok(builder.extension("return", vec![], loc)),
            Statement::Block(inner) => self.translate_block(inner, builder),
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        builder: &mut IntentBuilder<ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(builder.constant(*v, loc)),
            Expression::Literal(Literal::String(s)) => Ok(builder.string(s, loc)),
            Expression::Identifier(s) => Ok(builder.symbol(s, loc)),
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.translate_expr(arg, builder)?);
                }

                // Handle standard builtins
                if let Some(target) = &call.target {
                    if let Expression::Identifier(target_name) = &**target {
                        if target_name == "System.Console" {
                            if call.name == "WriteLine" {
                                return Ok(builder.cross_lang_call("nyar", "std::io::println", arg_ids, loc));
                            } else if call.name == "Write" {
                                return Ok(builder.cross_lang_call("nyar", "std::io::print", arg_ids, loc));
                            }
                        }
                    } else if let Expression::FieldAccess(fa) = &**target {
                        // Handle System.Console as FieldAccess if needed
                        if let Expression::Identifier(t) = &*fa.target {
                            if t == "System" && fa.name == "Console" {
                                if call.name == "WriteLine" {
                                    return Ok(builder.cross_lang_call("nyar", "std::io::println", arg_ids, loc));
                                } else if call.name == "Write" {
                                    return Ok(builder.cross_lang_call("nyar", "std::io::print", arg_ids, loc));
                                }
                            }
                        }
                    }
                } else {
                    // Handle cases where the full name is in call.name (due to parser combining identifiers)
                    if call.name == "System.Console.WriteLine" {
                        return Ok(builder.cross_lang_call("nyar", "std::io::println", arg_ids, loc));
                    } else if call.name == "System.Console.Write" {
                        return Ok(builder.cross_lang_call("nyar", "std::io::print", arg_ids, loc));
                    }
                }

                let name_id = builder.symbol(&call.name, loc);
                let args_id = builder.seq(arg_ids, loc);

                if let Some(target) = &call.target {
                    let target_id = self.translate_expr(target, builder)?;
                    Ok(builder.extension("call", vec![target_id, name_id, args_id], loc))
                } else {
                    Ok(builder.extension("call", vec![name_id, args_id], loc))
                }
            }
            Expression::FieldAccess(access) => {
                let target_id = self.translate_expr(&access.target, builder)?;
                let name_id = builder.symbol(&access.name, loc);
                Ok(builder.extension("field", vec![target_id, name_id], loc))
            }
            _ => Ok(builder.constant(0, loc)),
        }
    }
}
