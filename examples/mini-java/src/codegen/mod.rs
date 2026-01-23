//! Java 到 Nyar 字节码的翻译器

use anyhow::Result;
use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder, IKunTree};
use chomsky_extract::{Backend, BackendArtifact, IKunExtractor};
use chomsky_cost::DEFAULT_COST_MODEL;
use nyar_vm::bytecode::format::{Chunk, NyarModule, Constant as NyarConstant, ExportInfo};
use nyar_vm::bytecode::opcode::{Opcode, I32Ext, StringExt};
use oak_java::ast::*;

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

    fn lower_tree(&mut self, tree: &IKunTree) -> Result<Vec<u8>> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Constant(v) => {
                code.push(Opcode::Push as u8);
                let idx = self.add_constant(NyarConstant::Int(*v));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::StringConstant(s) => {
                code.push(Opcode::StringExt as u8);
                code.push(StringExt::Const as u8);
                let idx = self.add_constant(NyarConstant::String(s.clone()));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::Symbol(s) => {
                code.push(Opcode::LoadGlobal as u8);
                let idx = self.add_constant(NyarConstant::String(s.clone()));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::Seq(items) => {
                for item in items {
                    code.extend(self.lower_tree(item)?);
                }
            }
            IKunTree::Extension(name, args) => match name.as_str() {
                "class" => {
                    // 类处理：通常不需要生成代码，而是填充元数据
                    // args[0] 是类名, args[1] 是成员序列
                    if let IKunTree::StringConstant(class_name) = &args[0] {
                        if let IKunTree::Seq(members) = &args[1] {
                            for member in members {
                                self.lower_tree(member)?;
                            }
                        }
                    }
                }
                "method" => {
                    // 方法处理：生成 Chunk 并添加导出
                    if let (IKunTree::StringConstant(name), IKunTree::StringConstant(_ret), body) = (&args[0], &args[1], &args[2]) {
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
                "call" => {
                    // 调用处理
                    if args.len() == 3 {
                        // target, name, args
                        code.extend(self.lower_tree(&args[2])?); // push args
                        code.extend(self.lower_tree(&args[0])?); // push target
                        code.push(Opcode::InvokeMethod as u8);
                        let name_str = if let IKunTree::Symbol(s) = &args[1] { s } else { "unknown" };
                        let idx = self.add_constant(NyarConstant::String(name_str.to_string()));
                        code.extend_from_slice(&(idx as u16).to_le_bytes());
                        
                        // 获取参数数量
                        let arg_count = match &args[2] {
                            IKunTree::Seq(list) => list.len() as u8,
                            _ => 1,
                        };
                        code.push(arg_count);
                    } else if args.len() == 2 {
                        // name, args
                        code.extend(self.lower_tree(&args[1])?); // push args
                        code.push(Opcode::CallSymbol as u8);
                        let name_str = if let IKunTree::Symbol(s) = &args[0] { s } else { "unknown" };
                        let idx = self.add_constant(NyarConstant::String(name_str.to_string()));
                        code.extend_from_slice(&(idx as u16).to_le_bytes());
                        
                        let arg_count = match &args[1] {
                            IKunTree::Seq(list) => list.len() as u8,
                            _ => 1,
                        };
                        code.push(arg_count);
                    }
                }
                _ => {}
            },
            IKunTree::Module(_, items) => {
                for item in items {
                    self.lower_tree(item)?;
                }
            }
            _ => {
                // 其他类型暂不支持
            }
        }
        Ok(code)
    }

    fn add_constant(&mut self, c: NyarConstant) -> usize {
        if let Some(pos) = self.module.constants.iter().position(|x| x == &c) {
            pos
        } else {
            let pos = self.module.constants.len();
            self.module.constants.push(c);
            pos
        }
    }
}

impl Backend for NyarBackend {
    fn name(&self) -> &str {
        "nyar"
    }

    fn generate(&self, _tree: &IKunTree) -> chomsky_types::ChomskyResult<BackendArtifact> {
        // 这里实际上我们需要一个能修改 self 的版本，
        // 或者在 generate 中完成所有工作。
        // 由于 trait 定义是 &self，我们需要内部可变性或者重新设计。
        // 为了简单起见，我们在这里手动调用 lower_tree。
        Err(chomsky_types::ChomskyError::backend_error("Use NyarTranslator::translate instead"))
    }
}

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
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        // 提取最优路径
        let extractor = IKunExtractor::new(&egraph, DEFAULT_COST_MODEL.clone());
        let tree = extractor.extract(root_id);

        // 降级为 Nyar 字节码
        let mut backend = NyarBackend::new();
        backend.lower_tree(&tree)?;

        if backend.module.chunks.is_empty() && backend.module.constants.is_empty() {
            eprintln!("Warning: Generated NyarModule is empty. Tree: {:?}", tree);
        }

        Ok(backend.module)
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

        Ok(builder.seq(items, Loc::new(0, 0, 0)))
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

        let members_id = builder.seq(members, loc.clone());
        Ok(builder.extension("class", vec![name_id, members_id], loc))
    }

    fn translate_method(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, method: &MethodDeclaration) -> Result<Id> {
        let loc = Loc::new(0, method.span.start as u32, method.span.end as u32);
        let name_id = builder.string(&method.name, loc.clone());
        let ret_id = builder.string(&method.return_type, loc.clone());

        let mut body = Vec::new();
        for stmt in &method.body {
            body.push(self.translate_statement(builder, stmt)?);
        }

        let body_id = builder.seq(body, loc.clone());
        Ok(builder.extension("method", vec![name_id, ret_id, body_id], loc))
    }

    fn translate_statement(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, stmt: &Statement) -> Result<Id> {
        match stmt {
            Statement::Expression(expr) => self.translate_expression(builder, expr),
            Statement::Return(expr) => {
                let val = if let Some(e) = expr {
                    self.translate_expression(builder, e)?
                } else {
                    builder.constant(0, Loc::new(0, 0, 0))
                };
                Ok(builder.extension("return", vec![val], Loc::new(0, 0, 0)))
            }
            Statement::Block(stmts) => {
                let mut ids = Vec::new();
                for s in stmts {
                    ids.push(self.translate_statement(builder, s)?);
                }
                Ok(builder.seq(ids, Loc::new(0, 0, 0)))
            }
        }
    }

    fn translate_expression(&self, builder: &mut IntentBuilder<ConstraintAnalysis>, expr: &Expression) -> Result<Id> {
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Integer(i) => Ok(builder.constant(*i, Loc::new(0, 0, 0))),
                Literal::String(s) => Ok(builder.string(s, Loc::new(0, 0, 0))),
                Literal::Boolean(b) => Ok(builder.bool(*b, Loc::new(0, 0, 0))),
            },
            Expression::Identifier(id) => Ok(builder.symbol(id, Loc::new(0, 0, 0))),
            Expression::MethodCall(call) => {
                let mut args = Vec::new();
                for arg in &call.arguments {
                    args.push(self.translate_expression(builder, arg)?);
                }

                if let Some(target) = &call.target {
                    let target_id = self.translate_expression(builder, target)?;
                    let name_id = builder.symbol(&call.name, Loc::new(0, 0, 0));
                    let args_id = builder.seq(args, Loc::new(0, 0, 0));
                    Ok(builder.extension("call", vec![target_id, name_id, args_id], Loc::new(0, 0, 0)))
                } else {
                    let name_id = builder.symbol(&call.name, Loc::new(0, 0, 0));
                    let args_id = builder.seq(args, Loc::new(0, 0, 0));
                    Ok(builder.extension("call", vec![name_id, args_id], Loc::new(0, 0, 0)))
                }
            }
        }
    }
}
