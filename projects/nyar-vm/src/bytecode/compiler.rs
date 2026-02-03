use crate::bytecode::instruction::Instruction;
use crate::bytecode::format::{Chunk, ClassInfo, Constant, ExportInfo, NyarcModule};
use crate::bytecode::opcode::Opcode;
use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use nyar_types::{NyarError, QualifiedName};

pub struct NyarBackend {
    module: NyarcModule,
}

impl NyarBackend {
    pub fn new() -> Self {
        Self {
            module: NyarcModule::default(),
        }
    }

    pub fn lower_tree(&mut self, tree: &IKunTree) -> Result<Vec<u8>, NyarError> {
        let mut code = Vec::new();
        match tree {
            IKunTree::Module(_name, items) => {
                let mut module_code = Vec::new();
                for item in items {
                    module_code.extend(self.lower_tree(item)?);
                }
                if !module_code.is_empty() {
                    if module_code.last() != Some(&(Opcode::Return as u8)) {
                        module_code.push(Opcode::Return as u8);
                    }
                    self.module.chunks.push(Chunk {
                        locals: 32,
                        upvalues: 0,
                        max_stack: 64,
                        code: module_code,
                        handlers: vec![],
                        lines: vec![],
                        decoded: None,
                        hotness: std::sync::atomic::AtomicU32::new(0),
                    });
                }
            }
            IKunTree::Export(name, body) => {
                if let IKunTree::Lambda(_params, body) = &**body {
                    let body_code = self.lower_tree(body)?;
                    let mut final_code = body_code;
                    // Ensure Return at the end
                    if final_code.last() != Some(&(Opcode::Return as u8)) {
                        final_code.push(Opcode::Return as u8);
                    }

                    let chunk_idx = self.module.chunks.len();
                    if chunk_idx >= u16::MAX as usize {
                        return Err(NyarError::new(
                            0x1007,
                            nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                            nyar_types::SourceLocation::default(),
                        ));
                    }
                    let chunk_idx = chunk_idx as u16;
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
                        symbol: QualifiedName::from(name.as_str()),
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
                let idx = self.add_constant(Constant::QualifiedName(QualifiedName::from(s.as_str())));
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
            IKunTree::CrossLangCall(_lang, name, args) => {
                for arg in args {
                    code.extend(self.lower_tree(arg)?);
                }
                let name_idx = self.add_constant(Constant::String(name.clone()));
                code.extend_from_slice(&Instruction::FFICall(name_idx, args.len() as u8).encode());
            }
            IKunTree::Intrinsic(id, args) => {
                for arg in args {
                    code.extend(self.lower_tree(arg)?);
                }
                // Use FFICall with a constant that represents the intrinsic ID
                // We use a special naming convention or just a number in the constant pool
                let name = format!("$intrinsic:{}", id);
                let name_idx = self.add_constant(Constant::String(name));
                code.extend_from_slice(&Instruction::FFICall(name_idx, args.len() as u8).encode());
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
                        if let IKunTree::StringConstant(class_name) = &args[0] {
                            let mut fields = Vec::new();
                            if let IKunTree::Seq(members) = &args[1] {
                                for member in members {
                                    if let IKunTree::Extension(ext_name, ext_args) = member {
                                        if ext_name == "field" {
                                            if let IKunTree::StringConstant(field_name) = &ext_args[0] {
                                                fields.push(field_name.clone());
                                            }
                                        }
                                    }
                                    self.lower_tree(member)?;
                                }
                            }
                            self.add_class(class_name.clone(), fields);
                        }
                    }
                    "new" => {
                        if let IKunTree::StringConstant(class_name) = &args[0] {
                            let qn = QualifiedName::from(class_name.as_str());
                            let idx = self.module.classes.iter().position(|c| c.name == qn).map(|i| i as u16).ok_or_else(|| {
                                NyarError::new(
                                    0x1002,
                                    nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::IndexOutOfBounds(0)),
                                    nyar_types::SourceLocation::default(),
                                )
                            })?;

                            if let IKunTree::Seq(params) = &args[1] {
                                for param in params {
                                    code.extend(self.lower_tree(param)?);
                                }
                            }
                            code.extend_from_slice(&Instruction::NewObject(idx).encode());
                        }
                    }
                    "method" => {
                        if args.len() >= 4 {
                            let name_idx = 0;
                            let body_idx = 3; // [name, params, return_type, body]
                            if let (IKunTree::StringConstant(name), body) =
                                (&args[name_idx], &args[body_idx])
                            {
                                let body_code = self.lower_tree(body)?;
                                let mut final_code = body_code;
                                if final_code.last() != Some(&(Opcode::Return as u8)) {
                                    final_code.push(Opcode::Return as u8);
                                }
                                let chunk_idx = self.module.chunks.len();
                                if chunk_idx >= u16::MAX as usize {
                                    return Err(NyarError::new(
                                        0x1007,
                                        nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::LimitExceeded),
                                        nyar_types::SourceLocation::default(),
                                    ));
                                }
                                let chunk_idx = chunk_idx as u16;
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
                                    symbol: QualifiedName::from(name.as_str()),
                                    chunk_idx,
                                });
                            }
                        }
                    }
                    "call" => {
                        if args.len() == 3 {
                            // [target, name, args]
                            code.extend(self.lower_tree(&args[0])?); // target
                            if let IKunTree::Symbol(name) = &args[1] {
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                if let IKunTree::Seq(call_args) = &args[2] {
                                    for arg in call_args {
                                        code.extend(self.lower_tree(arg)?);
                                    }
                                    code.extend_from_slice(
                                        &Instruction::InvokeMethod(name_idx, call_args.len() as u8)
                                            .encode(),
                                    );
                                }
                            }
                        } else if args.len() == 2 {
                            // [name, args]
                            if let IKunTree::Symbol(name) = &args[0] {
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                if let IKunTree::Seq(call_args) = &args[1] {
                                    for arg in call_args {
                                        code.extend(self.lower_tree(arg)?);
                                    }
                                    code.extend_from_slice(
                                        &Instruction::Call(name_idx, call_args.len() as u8).encode(),
                                    );
                                }
                            }
                        }
                    }
                    "get_field" => {
                        if args.len() == 2 {
                            // [target, name]
                            code.extend(self.lower_tree(&args[0])?);
                            if let IKunTree::Symbol(name) = &args[1] {
                                let name_idx = self.add_constant(Constant::String(name.clone()));
                                code.extend_from_slice(&Instruction::GetField(name_idx).encode());
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

    fn add_constant(&mut self, c: Constant) -> u16 {
        if let Some(pos) = self.module.constants.iter().position(|x| x == &c) {
            pos as u16
        } else {
            let idx = self.module.constants.len();
            if idx >= u16::MAX as usize {
                panic!("Constant pool overflow");
            }
            let idx = idx as u16;
            self.module.constants.push(c);
            idx
        }
    }

    fn add_class(&mut self, name: String, fields: Vec<String>) -> u16 {
        let qn = QualifiedName::from(name.as_str());
        if let Some(pos) = self.module.classes.iter().position(|x| x.name == qn) {
            pos as u16
        } else {
            let idx = self.module.classes.len();
            if idx >= u16::MAX as usize {
                panic!("Class pool overflow");
            }
            let idx = idx as u16;
            self.module.classes.push(ClassInfo { name: qn, fields });
            idx
        }
    }
    pub fn finish(self) -> NyarcModule {
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
