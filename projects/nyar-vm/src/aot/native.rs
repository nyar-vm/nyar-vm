use chomsky_extract::{Backend, BackendArtifact};
use chomsky_types::ChomskyResult;
use chomsky_uir::IKunTree;
use gaia_types::helpers::Architecture;
use pe_assembler::helpers::PeBuilder;
use pe_assembler::types::SubsystemType;
use x86_64_assembler::builder::ProgramBuilder;
use x86_64_assembler::instruction::{Instruction, Operand, Register};

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
        // 为影子空间和第 5 个参数预留空间 (4 * 8 + 8 = 40)
        // 为了保持 16 字节对齐，我们分配 48 字节 (16 * 3)
        // 进入 entry 时 rsp 是 16 字节对齐的
        builder.add_instruction(Instruction::Sub {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(48, 32),
        });

        self.emit_tree(tree, &mut builder, &mut data_bytes)?;

        // 4. ExitProcess(0)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(0, 32),
        });
        // call ExitProcess (index 2 in imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 2),
        });

        // 恢复栈指针 (虽然 ExitProcess 不会返回，但为了代码完整性加上)
        builder.add_instruction(Instruction::Add {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(48, 32),
        });

        let code = builder.compile_instructions().map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("Assembler error: {:?}", e))
        })?;

        // 使用 PeBuilder 构建 EXE
        // 注意：导入顺序必须与代码中的调用顺序一致！
        // 1. GetStdHandle (index 0)
        // 2. WriteFile (index 1)
        // 3. ExitProcess (index 2)
        let mut pe = PeBuilder::new()
            .architecture(self.arch.clone())
            .subsystem(SubsystemType::Console)
            .import_function("kernel32.dll", "GetStdHandle") // index 0
            .import_function("kernel32.dll", "WriteFile") // index 1
            .import_function("kernel32.dll", "ExitProcess") // index 2
            .code(code);

        if !data_bytes.is_empty() {
            pe = pe.data(data_bytes);
        }

        let exe_bytes = pe.generate().map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("PE Builder error: {}", e))
        })?;

        Ok(BackendArtifact::Binary(exe_bytes))
    }
}

impl NativeBackend {
    fn emit_tree(
        &self,
        tree: &IKunTree,
        builder: &mut ProgramBuilder,
        data: &mut Vec<u8>,
    ) -> ChomskyResult<()> {
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
            IKunTree::Apply(func, args) => {
                if let IKunTree::Symbol(name) = &**func {
                    if name == "System.Console.WriteLine" {
                        if let Some(IKunTree::StringConstant(s)) = args.first() {
                            self.emit_write_line(s, builder, data)?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_write_line(
        &self,
        s: &str,
        builder: &mut ProgramBuilder,
        data: &mut Vec<u8>,
    ) -> ChomskyResult<()> {
        data.extend_from_slice(s.as_bytes());
        data.push(0);
        // 为 lpNumberOfBytesWritten 预留 4 字节
        data.extend_from_slice(&[0, 0, 0, 0]);

        // 1. GetStdHandle(STD_OUTPUT_HANDLE = -11)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(-11i64, 32),
        });
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 0),
        });

        // 2. WriteFile(hFile, lpBuffer, nNumberOfBytesToWrite, lpNumberOfBytesWritten, lpOverlapped)
        // hFile (rcx) = rax
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::RCX),
            src: Operand::reg(Register::RAX),
        });

        // lpBuffer (rdx) = [rip + disp32] -> .data start
        builder.add_instruction(Instruction::Lea {
            dst: Register::RDX,
            displacement: 0,
            rip_relative: true,
        });

        // nNumberOfBytesToWrite (r8) = s.len()
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::R8),
            src: Operand::imm(s.len() as i64, 32),
        });

        // lpNumberOfBytesWritten (r9) = [rip + disp32] -> .data + offset
        builder.add_instruction(Instruction::Lea {
            dst: Register::R9,
            displacement: 0,
            rip_relative: true,
        });

        // lpOverlapped (stack [rsp+32]) = NULL
        builder.add_instruction(Instruction::Mov {
            dst: Operand::mem(Some(Register::RSP), None, 1, 32),
            src: Operand::imm(0, 32),
        });

        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 1),
        });

        Ok(())
    }
}
