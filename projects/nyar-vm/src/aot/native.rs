use chomsky_extract::{Backend, BackendArtifact};
use chomsky_uir::IKunTree;
use gaia_types::helpers::Architecture;
use pe_assembler::helpers::PeBuilder;
use pe_assembler::types::SubsystemType;
use x86_64_assembler::builder::ProgramBuilder;
use x86_64_assembler::instruction::{Instruction, Operand, Register};
use chomsky_types::ChomskyResult;

pub struct NativeBackend {
    arch: Architecture,
}

impl NativeBackend {
    pub fn new() -> Self {
        Self {
            arch: Architecture::X86_64,
        }
    }
}

impl Backend for NativeBackend {
    fn name(&self) -> &str {
        "native-x86_64"
    }

    fn generate(&self, tree: &IKunTree) -> ChomskyResult<BackendArtifact> {
        let mut builder = ProgramBuilder::new(self.arch.clone());
        let mut data_bytes = Vec::new();
        
        // --- 简单的机器码生成逻辑 ---
        self.emit_tree(tree, &mut builder, &mut data_bytes)?;
        
        // 4. ExitProcess(0)
        // xor ecx, ecx -> 用 sub ecx, ecx 模拟或者 mov ecx, 0
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(0, 32),
        });
        // call ExitProcess (index 0 in kernel32 imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });

        let code = builder.compile_instructions()
            .map_err(|e| chomsky_types::ChomskyError::backend_error(format!("Assembler error: {:?}", e)))?;

        // 使用 PeBuilder 构建 EXE
        let mut pe = PeBuilder::new()
            .architecture(self.arch.clone())
            .subsystem(SubsystemType::Console)
            .import_function("kernel32.dll", "ExitProcess")    // index 0
            .import_function("kernel32.dll", "GetStdHandle")   // index 1
            .import_function("kernel32.dll", "WriteFile")      // index 2
            .code(code);
        
        if !data_bytes.is_empty() {
            pe = pe.data(data_bytes);
        }

        let exe_bytes = pe.generate()
            .map_err(|e| chomsky_types::ChomskyError::backend_error(format!("PE Builder error: {:?}", e)))?;

        Ok(BackendArtifact::Binary(exe_bytes))
    }
}

impl NativeBackend {
    fn emit_tree(&self, tree: &IKunTree, builder: &mut ProgramBuilder, data: &mut Vec<u8>) -> ChomskyResult<()> {
        match tree {
            IKunTree::Module(_, items) => {
                for item in items {
                    self.emit_tree(item, builder, data)?;
                }
            }
            IKunTree::Export(_, body) => {
                self.emit_tree(body, builder, data)?;
            }
            IKunTree::Lambda(_, body) => {
                self.emit_tree(body, builder, data)?;
            }
            IKunTree::Seq(items) => {
                for item in items {
                    self.emit_tree(item, builder, data)?;
                }
            }
            IKunTree::CrossLangCall(lang, func, args) if lang == "native" || lang == "csharp" => {
                if func == "System.Console.WriteLine" {
                    if let Some(IKunTree::StringConstant(s)) = args.first() {
                        self.emit_write_line(s, builder, data)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_write_line(&self, s: &str, builder: &mut ProgramBuilder, data: &mut Vec<u8>) -> ChomskyResult<()> {
        let string_offset = data.len();
        data.extend_from_slice(s.as_bytes());
        data.push(0);

        // 1. GetStdHandle(STD_OUTPUT_HANDLE = -11)
        // mov ecx, -11
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(-11, 32),
        });
        // call GetStdHandle (index 1)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });

        // 2. WriteFile(hFile, lpBuffer, nNumberOfBytesToWrite, lpNumberOfBytesWritten, lpOverlapped)
        // mov rcx, rax (hFile)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::RCX),
            src: Operand::reg(Register::RAX),
        });
        // mov rdx, string_offset
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::RDX),
            src: Operand::imm(string_offset as i64, 64),
        });
        // mov r8, string_length
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::R8),
            src: Operand::imm(s.len() as i64, 32),
        });
        // sub r9, r9 (清零)
        builder.add_instruction(Instruction::Sub {
            dst: Operand::reg(Register::R9),
            src: Operand::reg(Register::R9),
        });
        
        // call WriteFile (index 2)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });

        Ok(())
    }
}
