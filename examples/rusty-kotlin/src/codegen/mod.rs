//! Kotlin 到 Nyar 字节码的翻译器

use crate::{KotlinResult, KotlinError};
use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder, IKunTree};
use chomsky_extract::{Backend, BackendArtifact, IKunExtractor};
use chomsky_cost::DEFAULT_COST_MODEL;
use nyar_vm::bytecode::format::{Chunk, NyarModule, Constant as NyarConstant, ExportInfo};
use nyar_vm::bytecode::opcode::{Opcode, I32Ext, StringExt};
use oak_kotlin::ast::*;

pub struct NyarTranslator;

struct NyarBackend {
    module: NyarModule,
}

impl NyarBackend {
    fn new() -> Self {
        Self {
            module: NyarModule::default(),
        }
    }

    fn lower_tree(&mut self, tree: &IKunTree) -> KotlinResult<Vec<u8>> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Constant(v) => {
                println!("  LOWER: Push Int({})", v);
                code.push(Opcode::Push as u8);
                let idx = self.add_constant(NyarConstant::Int(*v));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::StringConstant(s) => {
                println!("  LOWER: Push String(\"{}\")", s);
                code.push(Opcode::StringExt as u8);
                code.push(StringExt::Const as u8);
                let idx = self.add_constant(NyarConstant::String(s.clone()));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::Symbol(s) => {
                println!("  LOWER: LoadGlobal(\"{}\")", s);
                code.push(Opcode::LoadGlobal as u8);
                let idx = self.add_constant(NyarConstant::String(s.clone()));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::Seq(items) => {
                for item in items {
                    code.extend(self.lower_tree(item)?);
                }
            }
            IKunTree::Extension(name, args) => {
                println!("Processing extension: {}", name);
                match name.as_str() {
                "class" => {
                    // 类处理：通常不需要生成代码，而是填充元数据
                    // args[0] 是类名, args[1] 是成员序列
                    if let IKunTree::StringConstant(class_name) = &args[0] {
                        println!("Found class: {}", class_name);
                        if let IKunTree::Seq(members) = &args[1] {
                            for member in members {
                                self.lower_tree(member)?;
                            }
                        }
                    }
                }
                "method" => {
                    // 方法处理：生成 Chunk 并添加导出
                    // args: [name, ret, params, body]
                    println!("Found method extension with {} args", args.len());
                    if args.len() == 4 {
                        if let (IKunTree::StringConstant(name), IKunTree::StringConstant(_ret), _params, body) = (&args[0], &args[1], &args[2], &args[3]) {
                            println!("Compiling method: {}", name);
                            let body_code = self.lower_tree(body)?;
                            let chunk_idx = self.module.chunks.len() as u16;
                            self.module.chunks.push(Chunk {
                                locals: 0,
                                upvalues: 0,
                                max_stack: 10,
                                code: body_code,
                                handlers: vec![],
                                lines: vec![],
                            });
                            self.module.exports.push(ExportInfo {
                                symbol: name.clone(),
                                chunk_idx,
                            });
                        }
                    } else if args.len() == 3 {
                         if let (IKunTree::StringConstant(name), IKunTree::StringConstant(_ret), body) = (&args[0], &args[1], &args[2]) {
                            println!("Compiling method (3 args): {}", name);
                            let body_code = self.lower_tree(body)?;
                            let chunk_idx = self.module.chunks.len() as u16;
                            self.module.chunks.push(Chunk {
                                locals: 0,
                                upvalues: 0,
                                max_stack: 10,
                                code: body_code,
                                handlers: vec![],
                                lines: vec![],
                            });
                            self.module.exports.push(ExportInfo {
                                symbol: name.clone(),
                                chunk_idx,
                            });
                        }
                    }
                }
                _ => {}
                }
            }
            _ => {}
        }
        Ok(code)
    }

    fn add_constant(&mut self, constant: NyarConstant) -> u16 {
        for (i, c) in self.module.constants.iter().enumerate() {
            if c == &constant {
                return i as u16;
            }
        }
        let idx = self.module.constants.len() as u16;
        self.module.constants.push(constant);
        idx
    }
}

impl Backend for NyarBackend {
    type Error = KotlinError;

    fn extract(&mut self, graph: &EGraph<IKun, ConstraintAnalysis>, root: Id) -> Result<BackendArtifact, Self::Error> {
        let extractor = IKunExtractor::new(graph, &DEFAULT_COST_MODEL);
        let tree = extractor.extract(root);
        
        println!("Extracted Tree: {:#?}", tree);
        
        self.lower_tree(&tree)?;

        let data = self.module.encode();
        Ok(BackendArtifact::Binary(data))
    }
}

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate(&self, _root: &KotlinRoot) -> KotlinResult<NyarModule> {
        // 对于 Mini Kotlin，我们先编译到 UIR，然后再从 UIR 提取
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        self.translate_to_graph(_root, &mut egraph)?;

        let mut backend = NyarBackend::new();
        let root_id = egraph.get_root().ok_or_else(|| KotlinError::Codegen("No root in egraph".to_string()))?;
        
        backend.extract(&egraph, root_id)?;
        Ok(backend.module)
    }

    pub fn translate_to_graph(&self, _root: &KotlinRoot, egraph: &mut EGraph<IKun, ConstraintAnalysis>) -> KotlinResult<Id> {
        let mut builder = IntentBuilder::new(egraph);
        
        // TODO: 真正的 Kotlin AST 到 UIR 的转换
        // 目前只是一个占位符，生成一个简单的 "Hello World" 方法
        
        let hello_str = builder.string("Hello from Mini Kotlin!");
        let print_sym = builder.symbol("println");
        let call = builder.extension("call", vec![print_sym, hello_str]);
        
        let main_body = builder.seq(vec![call]);
        let main_method = builder.extension("method", vec![
            builder.string("main"),
            builder.string("void"),
            builder.seq(vec![]), // params
            main_body
        ]);
        
        let class_members = builder.seq(vec![main_method]);
        let class_node = builder.extension("class", vec![
            builder.string("MainKt"),
            class_members
        ]);

        let root_id = builder.seq(vec![class_node]);
        builder.set_root(root_id);
        
        Ok(root_id)
    }
}
