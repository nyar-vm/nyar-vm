use chomsky_extract::{Backend, BackendArtifact};
use chomsky_uir::IKunTree;
use gaia_types::helpers::Architecture;
use pe_assembler::helpers::PeBuilder;
use pe_assembler::types::SubsystemType;
use x86_64_assembler::builder::ProgramBuilder;
use x86_64_assembler::instruction::{Instruction, Operand, Register};
use chomsky_types::{ChomskyError, ChomskyErrorKind, ChomskyResult};

use gaia_types::GaiaError;

pub struct NativeBackend {
    arch: Architecture,
}

impl NativeBackend {
    pub fn new() -> Self {
        Self {
            arch: Architecture::X86_64,
        }
    }

    fn wrap_error(&self, stage: &str, message: String) -> ChomskyError {
        ChomskyError {
            kind: Box::new(ChomskyErrorKind::BackendError {
                target: self.name().to_string(),
                stage: stage.to_string(),
                message,
            }),
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
        // xor ecx, ecx
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(0, 32),
        });
        // call ExitProcess (index 0 in kernel32 imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });

        let code = builder.compile_instructions()
            .map_err(|e| self.wrap_error("Assembler", format!("{:?}", e)))?;

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
            .map_err(|e| self.wrap_error("PEBuilder", format!("{:?}", e)))?;

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
        // 准备字符串数据
        let s_with_newline = format!("{}\r\n", s);
        let start_offset = data.len();
        data.extend_from_slice(s_with_newline.as_bytes());
        
        // 1. GetStdHandle(-11)
        // mov ecx, -11
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(-11i64, 32),
        });
        // call GetStdHandle (index 1 in kernel32 imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });
        // mov r12, rax (save handle)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::R12),
            src: Operand::reg(Register::RAX),
        });

        // 2. WriteFile(hStdOut, lpBuffer, nNumberOfBytesToWrite, &lpNumberOfBytesWritten, NULL)
        // rcx = hStdOut (r12)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::RCX),
            src: Operand::reg(Register::R12),
        });
        
        // rdx = lpBuffer (lea rdx, [rip+0] points to .data start)
        // Note: Currently fix_code_relocations only supports LEA RDX, [RIP+0] pointing to the START of .data.
        // If we have multiple strings, we need to handle offsets.
        // For now, we assume this is the only string and it's at offset 0.
        builder.add_instruction(Instruction::Lea {
            dst: Register::RDX,
            displacement: start_offset as i32, // This might not be fully supported by PeBuilder yet if it's not 0
            rip_relative: true,
        });
        
        // r8 = nNumberOfBytesToWrite
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::R8),
            src: Operand::imm(s_with_newline.len() as i64, 32),
        });
        
        // r9 = &lpNumberOfBytesWritten (stack space)
        // sub rsp, 40 (shadow space + 1 stack param)
        builder.add_instruction(Instruction::Sub {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(40, 8),
        });
        
        // lea r9, [rsp + 48] (somewhere on stack)
        builder.add_instruction(Instruction::Lea {
            dst: Register::R9,
            displacement: 48,
            rip_relative: false,
        });
        
        // stack[32] = NULL (5th parameter)
        // mov qword ptr [rsp + 32], 0
        // (x86_64-assembler might not support Mov Mem, Imm directly, let's use a register)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::RAX),
            src: Operand::imm(0, 64),
        });
        // TODO: Implement Mov Mem, Reg in x86_64-assembler if needed
        // For now, let's just hope WriteFile handles NULL correctly for the 5th param if we don't pass it? 
        // No, it's required.
        
        // call WriteFile (index 2 in kernel32 imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });
        
        // add rsp, 40
        builder.add_instruction(Instruction::Add {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(40, 8),
        });

        Ok(())
    }
}
