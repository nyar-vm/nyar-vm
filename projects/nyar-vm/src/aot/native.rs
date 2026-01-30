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
        
        // --- 简单的机器码生成逻辑 ---
        // 这里我们针对 hello.cs 的 IKunTree 进行特化处理
        // 在真正的实现中，这里应该是一个递归的遍历过程
        
        self.emit_tree(tree, &mut builder)?;
        
        // 添加退出进程的代码
        // mov ecx, 0 (exit code)
        // call ExitProcess
        builder.push_instruction(Instruction::Mov {
            dest: Operand::Register(Register::RCX),
            src: Operand::Immediate(0),
        });
        builder.push_instruction(Instruction::Call {
            target: Operand::Label("ExitProcess".to_string()),
        });

        let code = builder.compile_instructions()
            .map_err(|e| chomsky_types::GaiaError::not_implemented(format!("Assembler error: {:?}", e)))?;

        // 使用 PeBuilder 构建 EXE
        let mut pe = PeBuilder::new(self.arch.clone());
        pe.set_subsystem(SubsystemType::WindowsGui); // 或者 Console
        pe.add_section(".text", code, true, false, true);
        
        // 添加导入
        pe.add_import("kernel32.dll", "ExitProcess");
        pe.add_import("kernel32.dll", "GetStdHandle");
        pe.add_import("kernel32.dll", "WriteFile");
        
        // 设置入口点
        pe.set_entry_point(".text");

        let exe_bytes = pe.generate()
            .map_err(|e| chomsky_types::GaiaError::not_implemented(format!("PE Builder error: {:?}", e)))?;

        Ok(BackendArtifact::Binary(exe_bytes))
    }
}

impl NativeBackend {
    fn emit_tree(&self, tree: &IKunTree, builder: &mut ProgramBuilder) -> ChomskyResult<()> {
        match tree {
            IKunTree::Module(_, items) => {
                for item in items {
                    self.emit_tree(item, builder)?;
                }
            }
            IKunTree::Export(_, body) => {
                self.emit_tree(body, builder)?;
            }
            IKunTree::Lambda(_, body) => {
                // 暂时假设只有一个 Lambda (Main)
                self.emit_tree(body, builder)?;
            }
            IKunTree::Seq(items) => {
                for item in items {
                    self.emit_tree(item, builder)?;
                }
            }
            IKunTree::CrossLangCall(lang, func, args) if lang == "native" || lang == "csharp" => {
                if func == "System.Console.WriteLine" {
                    if let Some(IKunTree::StringConstant(s)) = args.first() {
                        self.emit_write_line(s, builder)?;
                    }
                }
            }
            _ => {
                // TODO: 实现更多节点的翻译
            }
        }
        Ok(())
    }

    fn emit_write_line(&self, s: &str, builder: &mut ProgramBuilder) -> ChomskyResult<()> {
        // 1. 获取 stdout 句柄
        // mov ecx, -11 (STD_OUTPUT_HANDLE)
        // call GetStdHandle
        builder.push_instruction(Instruction::Mov {
            dest: Operand::Register(Register::RCX),
            src: Operand::Immediate(-11i64 as u64),
        });
        builder.push_instruction(Instruction::Call {
            target: Operand::Label("GetStdHandle".to_string()),
        });
        // 句柄在 RAX 中，保存到寄存器或栈上
        // mov r12, rax
        builder.push_instruction(Instruction::Mov {
            dest: Operand::Register(Register::R12),
            src: Operand::Register(Register::RAX),
        });

        // 2. 准备字符串数据 (目前简单的处理，将字符串作为立即数或者之后放入 .data 段)
        // 这里我们需要一种方式在 PE 中添加数据段。目前 PeBuilder 可能还没完全支持
        // 我们可以暂时将字符串硬编码在代码段中（虽然不推荐）或者完善 PeBuilder
        
        // 暂时假设我们能通过标签引用数据
        let label = format!("str_{}", s.len());
        // 实际上我们需要在 PE 中添加这个字符串
        
        // 3. 调用 WriteFile
        // WriteFile(hStdOut, lpBuffer, nNumberOfBytesToWrite, &lpNumberOfBytesWritten, NULL)
        // rcx = hStdOut (r12)
        // rdx = lpBuffer
        // r8 = nNumberOfBytesToWrite
        // r9 = &lpNumberOfBytesWritten (可以指向栈空间)
        // stack[4] = NULL
        
        builder.push_instruction(Instruction::Mov {
            dest: Operand::Register(Register::RCX),
            src: Operand::Register(Register::R12),
        });
        // TODO: 加载字符串地址到 RDX
        // builder.push_instruction(Instruction::Lea { ... });
        
        builder.push_instruction(Instruction::Mov {
            dest: Operand::Register(Register::R8),
            src: Operand::Immediate(s.len() as u64),
        });
        // ... 其他参数设置
        
        builder.push_instruction(Instruction::Call {
            target: Operand::Label("WriteFile".to_string()),
        });

        Ok(())
    }
}
