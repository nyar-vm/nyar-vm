//! Java 到 Nyar 字节码的翻译器

use anyhow::Result;
use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder};
use nyar_vm::bytecode::format::{Chunk, NyarModule};
use oak_java::ast::*;

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_to_graph(&self, ast: &JavaRoot, egraph: &mut EGraph<IKun, ConstraintAnalysis>) -> Result<Id> {
        let mut builder = IntentBuilder::new(egraph);
        self.translate_root(&mut builder, ast)
    }

    pub fn translate(&self, ast: &JavaRoot) -> Result<NyarModule> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let _root_id = self.translate_to_graph(ast, &mut egraph)?;

        // TODO: 提取最优路径并降级为 Nyar 字节码
        let mut module = NyarModule::default();
        let main_chunk = Chunk {
            locals: 0,
            upvalues: 0,
            max_stack: 10,
            code: vec![],
            handlers: vec![],
            lines: vec![],
        };
        module.chunks.push(main_chunk);

        Ok(module)
    }

    fn translate_root(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, ast: &JavaRoot) -> Result<Id> {
        let mut items = Vec::new();
        for item in &ast.items {
            let id = match item {
                Item::Class(c) => self.translate_class(builder, c)?,
                Item::Interface(i) => {
                    let loc = Loc::new(0, i.span.start as u32, i.span.end as u32);
                    let name_id = builder.string(&i.name, loc.clone());
                    builder.extension("interface", vec![name_id], loc)
                }
                Item::Package(p) => {
                    let loc = Loc::new(0, p.span.start as u32, p.span.end as u32);
                    let name_id = builder.string(&p.name, loc.clone());
                    builder.extension("package", vec![name_id], loc)
                }
                Item::Import(i) => {
                    let loc = Loc::new(0, i.span.start as u32, i.span.end as u32);
                    let path_id = builder.string(&i.path, loc.clone());
                    builder.extension("import", vec![path_id], loc)
                }
            };
            items.push(id);
        }
        Ok(builder.module("mini-java-program", items))
    }

    fn translate_class(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, class: &ClassDeclaration) -> Result<Id> {
        let loc = Loc::new(0, class.span.start as u32, class.span.end as u32);
        let name_id = builder.string(&class.name, loc.clone());

        let mut members = Vec::new();
        for member in &class.members {
            let member_id = match member {
                Member::Method(m) => self.translate_method(builder, m)?,
                Member::Field(_) => continue, // 暂不支持字段
            };
            members.push(member_id);
        }

        Ok(builder.extension("class", vec![name_id, builder.seq(members)], loc))
    }

    fn translate_method(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, method: &MethodDeclaration) -> Result<Id> {
        let loc = Loc::new(0, method.span.start as u32, method.span.end as u32);

        let mut body_ids = Vec::new();
        for stmt in &method.body {
            body_ids.push(self.translate_statement(builder, stmt)?);
        }
        let body_id = builder.seq(body_ids);

        let params: Vec<String> = method.parameters.iter().map(|p| p.name.clone()).collect();
        let lambda_id = builder.lambda(params, body_id);

        Ok(builder.export(&method.name, lambda_id))
    }

    fn translate_statement(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, stmt: &Statement) -> Result<Id> {
        match stmt {
            Statement::Expression(expr) => self.translate_expression(builder, expr),
            Statement::Return(expr) => {
                let val_id = if let Some(e) = expr {
                    self.translate_expression(builder, e)?
                } else {
                    builder.constant(0) // Default return
                };
                Ok(builder.extension("return", vec![val_id], Loc::default()))
            }
            Statement::Block(stmts) => {
                let mut ids = Vec::new();
                for s in stmts {
                    ids.push(self.translate_statement(builder, s)?);
                }
                Ok(builder.seq(ids))
            }
        }
    }

    fn translate_expression(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, expr: &Expression) -> Result<Id> {
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Integer(i) => Ok(builder.constant(*i)),
                Literal::String(s) => Ok(builder.string(s, Loc::default())),
                Literal::Boolean(b) => Ok(builder.boolean(*b)),
            },
            Expression::Identifier(id) => Ok(builder.symbol(id)),
            Expression::MethodCall(call) => {
                let mut args = Vec::new();
                for arg in &call.arguments {
                    args.push(self.translate_expression(builder, arg)?);
                }

                if let Some(target) = &call.target {
                    let target_id = self.translate_expression(builder, target)?;
                    Ok(builder.extension("call", vec![target_id, builder.symbol(&call.name), builder.seq(args)], Loc::default()))
                } else {
                    Ok(builder.extension("call", vec![builder.symbol(&call.name), builder.seq(args)], Loc::default()))
                }
            }
        }
    }
}
