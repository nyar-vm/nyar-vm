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
        // 对于真正的入口点（Entry Point），进入时 RSP 通常是 16 字节对齐的（RSP = 16n）。
        // 我们需要分配一定的栈空间，使得在调用其他函数前，RSP 依然是 16 字节对齐的。
        // 即 (RSP - stack_size) % 16 == 0。
        // 所以 stack_size 必须是 16 的倍数。
        // 同时，Windows 要求至少 32 字节的影子空间。
        let mut offset = 32; // 至少预留 32 字节影子空间
        for local in locals {
            context.locals.insert(local, offset);
            offset += 8;
        }
        // 向上对齐到 16 的倍数再加 8，以确保 call 时的 RSP 为 16 字节对齐
        // (RSP_entry = 16n + 8, RSP_call = RSP_entry - stack_size = 16n + 8 - (16k + 8) = 16m)
        context.stack_size = ((offset + 15) & !15) + 8;

        // 2. 函数序言 (Prologue)
        builder.add_instruction(Instruction::Sub {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(context.stack_size as i64, 32),
        });

        // 3. Body
        self.emit_tree(tree, &mut builder, &mut data_bytes, &mut context)?;

        // 4. 函数尾声 (Epilogue)
        // 所有 Return 会跳转到这里。在这里我们不需要还原 RSP，因为我们要直接调用 ExitProcess。
        // 如果是真正的函数调用，则需要 Add RSP, context.stack_size 并 Ret。
        builder.add_instruction(Instruction::Label("epilogue".to_string()));

        // 5. 退出进程 (Terminate)
        // 使用 rax 作为退出码调用 ExitProcess
        builder.add_instruction(Instruction::Label("terminate".to_string()));
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::reg(Register::EAX),
        });
        // call ExitProcess (index 2 in imports)
        // 注意：此时 RSP 依然是 16 字节对齐的，且下方有足够的影子空间。
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 2),
        });

        // 理论上不会执行到这里，但为了保险起见，还原栈并返回
        builder.add_instruction(Instruction::Label("exit_cleanup".to_string()));
        builder.add_instruction(Instruction::Add {
            dst: Operand::reg(Register::RSP),
            src: Operand::imm(context.stack_size as i64, 32),
        });
        builder.add_instruction(Instruction::Ret);

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
            IKunTree::Source(_, body) => {
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
                        src: Operand::mem(Some(Register::RSP), None, 0, offset),
                    });
                } else {
                    eprintln!("Warning: Unresolved symbol {}", name);
                }
            }
            IKunTree::Source(_, body) => {
                self.emit_tree(body, builder, data, context)?;
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
                builder.add_instruction(Instruction::Jmp {
                    target: Operand::label("epilogue".to_string()),
                });
            }
            IKunTree::Apply(func, _) => {
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
                        builder.add_instruction(Instruction::Jmp {
                            target: Operand::label("epilogue".to_string()),
                        });
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
                        // General call handling
                        let args_list = args.last().unwrap();
                        if let IKunTree::Seq(actual_args) = args_list {
                            for arg in actual_args {
                                self.emit_tree(arg, builder, data, context)?;
                            }
                        }
                        self.emit_tree(&args[0], builder, data, context)?;
                        builder.add_instruction(Instruction::Call {
                            target: Operand::reg(Register::RAX),
                        });
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
            IKunTree::CrossLangCall {
                language: lang,
                module_path: group,
                function_name: func,
                arguments: args,
            } => {
                if lang == "nyar" {
                    // Map to intrinsic logic for AOT
                    match (group.as_str(), func.as_str()) {
                        ("io", "println") | ("", "println") => {
                            // Println
                            if let Some(IKunTree::StringConstant(s)) = args.first() {
                                self.emit_write(s, true, builder, data)?;
                            }
                        }
                        ("io", "print") | ("", "print") => {
                            // Print
                            if let Some(IKunTree::StringConstant(s)) = args.first() {
                                self.emit_write(s, false, builder, data)?;
                            }
                        }
                        ("std", "exit") | ("", "exit") => {
                            // Exit
                            if let Some(IKunTree::Constant(code)) = args.first() {
                                self.emit_exit(*code as i32, builder)?;
                            } else {
                                self.emit_exit(0, builder)?;
                            }
                        }
                        ("std", "panic") | ("", "panic") => {
                            if let Some(IKunTree::StringConstant(s)) = args.first() {
                                self.emit_write(s, true, builder, data)?;
                            }
                            self.emit_exit(1, builder)?;
                        }
                        _ => {}
                    }
                } else if lang == "native" || lang == "csharp" {
                    // Avoid unused variable warning if we don't use func
                    let _ = func;
                    self.emit_printf(args, builder, data, context)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_write(
        &self,
        s: &str,
        newline: bool,
        builder: &mut ProgramBuilder,
        data: &mut Vec<u8>,
    ) -> ChomskyResult<()> {
        let mut full_s = s.to_string();
        if newline {
            full_s.push_str("\r\n");
        }
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

    fn emit_exit(&self, code: i32, builder: &mut ProgramBuilder) -> ChomskyResult<()> {
        builder.add_instruction(Instruction::Mov {
            dst: Operand::reg(Register::ECX),
            src: Operand::imm(code as i64, 32),
        });
        builder.add_instruction(Instruction::Call {
            target: Operand::mem(None, None, 0, 2),
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
