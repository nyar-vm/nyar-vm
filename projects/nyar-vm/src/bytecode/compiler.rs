use crate::bytecode::format::{Chunk, ExportInfo, NyarModule, Constant as NyarConstant};
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
            IKunTree::Extension(name, args) => {
                match name.as_str() {
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
                        if args.len() >= 3 {
                            let name_idx = args.len() - 3;
                            let body_idx = args.len() - 1;
                            if let (IKunTree::StringConstant(name), body) = (&args[name_idx], &args[body_idx]) {
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
        backend.lower_tree(tree).map_err(|e| chomsky_types::ChomskyError::backend_error(format!("{:?}", e)))?;
        let module = backend.finish();
        
        let json = serde_json::to_string_pretty(&module).map_err(|e| chomsky_types::ChomskyError::backend_error(e.to_string()))?;
        Ok(BackendArtifact::Source(json))
    }
}
