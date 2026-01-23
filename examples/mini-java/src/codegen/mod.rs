//! Java 到 Nyar 字节码的翻译器

use anyhow::Result;
use chomsky_uir::{EGraph, Id, ConstraintAnalysis, IKun, IntentBuilder};
use chomsky_source::Loc;
use nyar_vm::bytecode::format::{Chunk, NyarModule};
use oak_java::ast::{Item, JavaRoot};

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
        // 目前暂时返回一个空的模块，用于演示管线已连接
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
                Item::Class(c) => {
                    let loc = Loc::new(0, c.span.start as u32, c.span.end as u32);
                    let name_id = builder.string(&c.name, loc.clone());
                    builder.extension("class", vec![name_id], loc)
                }
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
}
