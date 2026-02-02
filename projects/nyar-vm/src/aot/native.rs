use chomsky_extract::{Backend, BackendArtifact};
use chomsky_types::ChomskyResult;
use chomsky_uir::IKunTree;
use gaia_types::helpers::Architecture;
use pe_assembler::helpers::PeBuilder;
use pe_assembler::types::SubsystemType;
use x86_64_assembler::builder::ProgramBuilder;
use x86_64_assembler::instruction::{Instruction, Operand, Register};
use std::collections::{HashMap, HashSet};

struct AotContext {
    locals: HashMap<String, i32>,
    stack_size: i32,
}


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

        // 1. 收集所有局部变量并计算栈大小
        let mut locals = HashSet::new();
        self.collect_locals(tree, &mut locals);

        let mut context = AotContext {
            locals: HashMap::new(),
            stack_size: 0,
        };

        // 为每个变量分配 8 字节空间
        // 栈布局：[Shadow Space (32)] [Locals...]
        // Windows x64 ABI 要求在 call 之前栈必须 16 字节对齐。
        // 进入函数时，由于返回地址压栈，RSP = 16n + 8。
        // 因此我们分配的 stack_size 必须满足 (RSP - stack_size) % 16 == 0，即 stack_size = 16k + 8。
        let mut offset = 32; // 至少预留 32 字节影子空间
        for local in locals {
            context.locals.insert(local, offset);
            offset += 8;
        }
        // 计算对齐后的 stack_size，确保其结尾为 8
        // 首先向上对齐到 16 的倍数，然后加 8
        context.stack_size = ((offset + 15) & !15) + 8;

        // 2. 函数序言 (Prologue)
        builder.add_instruction(Instruction::Sub {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(context.stack_size as i64, 32),
        });

        // 3. 生成代码
        self.emit_tree(tree, &mut builder, &mut data_bytes, &mut context)?;

        // 4. ExitProcess(rax)
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::reg(Register::EAX),
        });
        // call ExitProcess (index 2 in imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 2),
        });

        // 5. 函数尾声 (Epilogue)
        builder.add_instruction(Instruction::Add {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(context.stack_size as i64, 32),
        });

        let code = builder.compile_instructions().map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("Assembler error: {:?}", e))
        })?;

        // 使用 PeBuilder 构建 EXE
        let mut pe = PeBuilder::new()
            .architecture(self.arch.clone())
            .subsystem(SubsystemType::Console)
            .import_function("kernel32.dll", "GetStdHandle") // index 0
            .import_function("kernel32.dll", "WriteFile") // index 1
            .import_function("kernel32.dll", "ExitProcess") // index 2
            .import_function("msvcrt.dll", "printf") // index 3
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
    fn collect_locals(&self, tree: &IKunTree, locals: &mut HashSet<String>) {
        match tree {
            IKunTree::Module(_, items) => {
                for item in items {
                    self.collect_locals(item, locals);
                }
            }
            IKunTree::Seq(items) => {
                for item in items {
                    self.collect_locals(item, locals);
                }
            }
            IKunTree::StateUpdate(target, value) => {
                if let IKunTree::Symbol(name) = &**target {
                    locals.insert(name.clone());
                }
                self.collect_locals(value, locals);
            }
            IKunTree::Lambda(params, body) => {
                for param in params {
                    locals.insert(param.clone());
                }
                self.collect_locals(body, locals);
            }
            IKunTree::Apply(func, args) => {
                self.collect_locals(func, locals);
                for arg in args {
                    self.collect_locals(arg, locals);
                }
            }
            IKunTree::Extension(_, args) => {
                for arg in args {
                    self.collect_locals(arg, locals);
                }
            }
            IKunTree::Return(val) => {
                self.collect_locals(val, locals);
            }
            IKunTree::Export(_, body) => {
                self.collect_locals(body, locals);
            }
            _ => {}
        }
    }

    fn emit_tree(
        &self,
        tree: &IKunTree,
        builder: &mut ProgramBuilder,
        data: &mut Vec<u8>,
        context: &mut AotContext,
    ) -> ChomskyResult<()> {
        match tree {
            IKunTree::Module(_, items) => {
                for item in items {
                    self.emit_tree(item, builder, data, context)?;
                }
            }
            IKunTree::Constant(val) => {
                builder.add_instruction(Instruction::Mov {
                    dst: Operand::reg(Register::RAX),
                    src: Operand::imm(*val, 64),
                });
            }
            IKunTree::StringConstant(s) => {
                let offset = data.len();
                data.extend_from_slice(s.as_bytes());
                data.push(0);
                builder.add_instruction(Instruction::Lea {
                    dst: Register::RAX,
                    displacement: offset as i32,
                    rip_relative: true,
                });
            }
            IKunTree::Symbol(name) => {
                if let Some(&offset) = context.locals.get(name) {
                    builder.add_instruction(Instruction::Mov {
                        dst: Operand::reg(Register::RAX),
                        src: Operand::mem(Some(Register::RSP), None, 1, offset),
                    });
                } else {
                    eprintln!("Warning: Unresolved symbol {}", name);
                }
            }
            IKunTree::StateUpdate(target, value) => {
                self.emit_tree(value, builder, data, context)?;
                if let IKunTree::Symbol(name) = &**target {
                    if let Some(&offset) = context.locals.get(name) {
                        builder.add_instruction(Instruction::Mov {
                            dst: Operand::mem(Some(Register::RSP), None, 1, offset),
                            src: Operand::reg(Register::RAX),
                        });
                    }
                }
            }
            IKunTree::Export(_, body) => {
                self.emit_tree(body, builder, data, context)?;
            }
            IKunTree::Lambda(_, body) => {
                self.emit_tree(body, builder, data, context)?;
            }
            IKunTree::Return(val) => {
                self.emit_tree(val, builder, data, context)?;
            }
            IKunTree::Apply(func, args) => {
                if let IKunTree::Symbol(name) = &**func {
                    if let Some(builtin) = nyar_types::NyarBuiltin::from_cross_lang_call("native", name) {
                        match builtin {
                            nyar_types::NyarBuiltin::Println => {
                                if let Some(IKunTree::StringConstant(s)) = args.first() {
                                    self.emit_write_line(s, builder, data)?;
                                }
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
                // TODO: 真正的函数调用需要处理参数传递（RCX, RDX, R8, R9, Stack）
                for arg in args {
                    self.emit_tree(arg, builder, data, context)?;
                }
                self.emit_tree(func, builder, data, context)?;
                builder.add_instruction(Instruction::Call {
                    target: Operand::reg(Register::RAX),
                });
            }
            IKunTree::Extension(name, args) => {
                match name.as_str() {
                    "return" => {
                        if let Some(val) = args.first() {
                            self.emit_tree(val, builder, data, context)?;
                        }
                    }
                    "+" | "-" | "*" | "/" => {
                        if args.len() == 2 {
                            self.emit_tree(&args[0], builder, data, context)?;
                            builder.add_instruction(Instruction::Push {
                                op: Operand::reg(Register::RAX),
                            });
                            self.emit_tree(&args[1], builder, data, context)?;
                            builder.add_instruction(Instruction::Mov {
                                dst: Operand::reg(Register::RCX),
                                src: Operand::reg(Register::RAX),
                            });
                            builder.add_instruction(Instruction::Pop {
                                dst: Operand::reg(Register::RAX),
                            });

                            match name.as_str() {
                                "+" => {
                                    builder.add_instruction(Instruction::Add {
                                        dst: Operand::reg(Register::RAX),
                                        src: Operand::reg(Register::RCX),
                                    });
                                }
                                "-" => {
                                    builder.add_instruction(Instruction::Sub {
                                        dst: Operand::reg(Register::RAX),
                                        src: Operand::reg(Register::RCX),
                                    });
                                }
                                "*" => {
                                    builder.add_instruction(Instruction::Imul {
                                        dst: Register::RAX,
                                        src: Operand::reg(Register::RCX),
                                    });
                                }
                                "/" => {
                                    builder.add_instruction(Instruction::Cqo);
                                    builder.add_instruction(Instruction::Idiv {
                                        src: Operand::reg(Register::RCX),
                                    });
                                }
                                _ => {
                                    unreachable!()
                                }
                            }
                        }
                    }
                    "call" => {
                        // eprintln!("DEBUG: Extension call args: {:?}", args);
                        let is_write_line = if args.len() >= 3 {
                            let method_name = if let IKunTree::Symbol(name) = &args[1] {
                                name.as_str()
                            } else {
                                ""
                            };
                            
                            let target_is_console = match &args[0] {
                                IKunTree::Symbol(name) => name == "System.Console",
                                IKunTree::Extension(ext_name, ext_args) if ext_name == "field" => {
                                    if ext_args.len() == 2 {
                                        if let (IKunTree::Symbol(t), IKunTree::Symbol(n)) = (&ext_args[0], &ext_args[1]) {
                                            t == "System" && n == "Console"
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            };
                            
                            target_is_console && method_name == "WriteLine"
                        } else if args.len() >= 2 {
                            if let IKunTree::Symbol(method_name) = &args[0] {
                                method_name == "System.Console.WriteLine"
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if is_write_line {
                            let args_list = args.last().unwrap();
                            if let IKunTree::Seq(actual_args) = args_list {
                                if let Some(IKunTree::StringConstant(s)) = actual_args.first() {
                                    self.emit_write_line(s, builder, data)?;
                                }
                            }
                        }
                    }
                    "class" => {
                        if let Some(members) = args.get(1) {
                            self.emit_tree(members, builder, data, context)?;
                        }
                    }
                    "method" => {
                        if let Some(body) = args.get(2) {
                            self.emit_tree(body, builder, data, context)?;
                        }
                    }
                    _ => {}
                }
            }
            IKunTree::Seq(items) => {
                for item in items {
                    self.emit_tree(item, builder, data, context)?;
                }
            }
            IKunTree::CrossLangCall(lang, func, args) => {
                if lang == "nyar" {
                    match func.as_str() {
                        "println" => {
                            if let Some(IKunTree::StringConstant(s)) = args.first() {
                                self.emit_write_line(s, builder, data)?;
                            }
                        }
                        "print" => {
                            // TODO: Implement Print for AOT
                        }
                        "exit" => {
                            // TODO: Implement Exit for AOT
                        }
                        _ => {}
                    }
                } else if lang == "native" || lang == "csharp" {
                    self.emit_printf(args, builder, data, context)?;
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
        let mut full_s = s.to_string();
        full_s.push_str("\r\n");
        let s = full_s;

        let string_offset = data.len();
        data.extend_from_slice(s.as_bytes());
        data.push(0);
        // 为 lpNumberOfBytesWritten 预留 4 字节
        let written_offset = data.len();
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

        // lpBuffer (rdx) = [rip + disp32] -> string_offset
        builder.add_instruction(Instruction::Lea {
            dst: Register::RDX,
            displacement: string_offset as i32,
            rip_relative: true,
        });

        // nNumberOfBytesToWrite (r8) = s.len()
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::R8),
            src: Operand::imm(s.len() as i64, 32),
        });

        // lpNumberOfBytesWritten (r9) = [rip + disp32] -> written_offset
        builder.add_instruction(Instruction::Lea {
            dst: Register::R9,
            displacement: written_offset as i32,
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

    fn emit_printf(
        &self,
        args: &[IKunTree],
        builder: &mut ProgramBuilder,
        data: &mut Vec<u8>,
        context: &mut AotContext,
    ) -> ChomskyResult<()> {
        // Windows x64 calling convention for printf (variadic):
        // RCX, RDX, R8, R9, then stack.
        // For variadic, float arguments also go to XMM registers (not handled here yet).

        let arg_regs = [Register::RCX, Register::RDX, Register::R8, Register::R9];

        for (i, arg) in args.iter().enumerate() {
            self.emit_tree(arg, builder, data, context)?;
            if i < 4 {
                builder.add_instruction(Instruction::Mov {
                    dst: Operand::reg(arg_regs[i]),
                    src: Operand::reg(Register::RAX),
                });
            } else {
                // Push to stack (beyond shadow space)
                // RSP + 32 + (i-4)*8
                builder.add_instruction(Instruction::Mov {
                    dst: Operand::mem(Some(Register::RSP), None, 1, 32 + (i as i32 - 4) * 8),
                    src: Operand::reg(Register::RAX),
                });
            }
        }

        // Call printf (index 3 in imports)
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 3),
        });

        Ok(())
    }
}
