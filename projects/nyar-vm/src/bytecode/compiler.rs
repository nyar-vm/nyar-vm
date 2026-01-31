use crate::bytecode::decoder::Instruction;
use crate::bytecode::format::{Chunk, Constant as NyarConstant, ExportInfo, NyarModule};
use crate::bytecode::opcode::{Opcode, StringExt};
use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use nyar_types::VmError;

pub struct NyarBackend {
    module: NyarModule,
}

impl NyarBackend {
    pub fn new() -> Self {
        Self {
            module: NyarModule::default(),
        }
    }

    pub fn lower_tree(&mut self, tree: &IKunTree) -> Result<Vec<u8>, VmError> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Module(_name, items) => {
                for item in items {
                    self.lower_tree(item)?;
                }
            }
            IKunTree::Export(name, body) => {
                if let IKunTree::Lambda(params, body) = &**body {
                    let body_code = self.lower_tree(body)?;
                    let mut final_code = body_code;
                    // Ensure Return at the end
                    if final_code.last() != Some(&(Opcode::Return as u8)) {
                        final_code.push(Opcode::Return as u8);
                    }

                    let chunk_idx = self.module.chunks.len() as u16;
                    self.module.chunks.push(Chunk {
                        locals: 32,
                        upvalues: 0,
                        max_stack: 64,
                        code: final_code,
                        handlers: vec![],
                        lines: vec![],
                        decoded: None,
                        hotness: std::sync::atomic::AtomicU32::new(0),
                    });
                    self.module.exports.push(ExportInfo {
                        symbol: name.clone(),
                        chunk_idx,
                    });
                }
            }
            IKunTree::Constant(v) => {
                code.extend_from_slice(&Instruction::I64Const(*v).encode());
            }
            IKunTree::StringConstant(s) => {
                code.extend_from_slice(&Instruction::StringConst(s.clone()).encode());
            }
            IKunTree::Symbol(s) => {
                code.push(Opcode::LoadGlobal as u8);
                let idx = self.add_constant(NyarConstant::String(s.clone()));
                code.extend_from_slice(&(idx as u16).to_le_bytes());
            }
            IKunTree::Return(val) => {
                code.extend(self.lower_tree(val)?);
                code.push(Opcode::Return as u8);
            }
            IKunTree::Seq(items) => {
                for item in items {
                    code.extend(self.lower_tree(item)?);
                }
            }
            IKunTree::CrossLangCall(lang, name, args) => {
                if lang == "native" {
                    for arg in args {
                        code.extend(self.lower_tree(arg)?);
                    }
                    let name_idx = self.add_constant(NyarConstant::String(name.clone()));
                    code.extend_from_slice(
                        &Instruction::FFICall(name_idx, args.len() as u8).encode(),
                    );
                }
            }
            IKunTree::Extension(name, args) => {
                println!("Backend: Extension {}, args len {}", name, args.len());
                match name.as_str() {
                    "return" => {
                        if let Some(val) = args.first() {
                            code.extend(self.lower_tree(val)?);
                        }
                        code.push(Opcode::Return as u8);
                    }
                    "class" => {
                        if let IKunTree::StringConstant(_class_name) = &args[0] {
                            if let IKunTree::Seq(members) = &args[1] {
                                for member in members {
                                    self.lower_tree(member)?;
                                }
                            }
                        }
                    }
                    "method" => {
                        if args.len() >= 4 {
                            let name_idx = 0;
                            let body_idx = args.len() - 1;
                            if let (IKunTree::StringConstant(name), body) =
                                (&args[name_idx], &args[body_idx])
                            {
                                let body_code = self.lower_tree(body)?;
                                let mut final_code = body_code;
                                if final_code.last() != Some(&(Opcode::Return as u8)) {
                                    final_code.push(Opcode::Return as u8);
                                }
                                let chunk_idx = self.module.chunks.len() as u16;
                                self.module.chunks.push(Chunk {
                                    locals: 32,
                                    upvalues: 0,
                                    max_stack: 64,
                                    code: final_code,
                                    handlers: vec![],
                                    lines: vec![],
                                    decoded: None,
                                    hotness: std::sync::atomic::AtomicU32::new(0),
                                });
                                self.module.exports.push(ExportInfo {
                                    symbol: name.clone(),
                                    chunk_idx,
                                });
                            }
                        }
                    }
                    _ => {
                        // Handle other extensions or fallback
                    }
                }
            }
            _ => {}
        }
        Ok(code)
    }

    fn add_constant(&mut self, c: NyarConstant) -> u16 {
        if let Some(pos) = self.module.constants.iter().position(|x| x == &c) {
            pos as u16
        } else {
            let idx = self.module.constants.len() as u16;
            self.module.constants.push(c);
            idx
        }
    }
    pub fn finish(self) -> NyarModule {
        self.module
    }
}

impl Backend for NyarBackend {
    fn name(&self) -> &str {
        "nyar"
    }

    fn get_model(&self) -> &dyn chomsky_cost::CostModel {
        &chomsky_cost::DEFAULT_COST_MODEL
    }

    fn generate(&self, tree: &IKunTree) -> Result<BackendArtifact, chomsky_types::ChomskyError> {
        let mut backend = NyarBackend::new();
        backend
            .lower_tree(tree)
            .map_err(|e| chomsky_types::ChomskyError::backend_error(format!("{:?}", e)))?;
        let module = backend.finish();

        let json = serde_json::to_string_pretty(&module)
            .map_err(|e| chomsky_types::ChomskyError::backend_error(e.to_string()))?;
        Ok(BackendArtifact::Source(json))
    }
}
