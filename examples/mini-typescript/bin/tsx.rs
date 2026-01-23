use clap::Parser;
use std::fs;
use mini_typescript::MiniTypescriptFrontend;
use nyar_vm::NyarVM;
use nyar_vm::bytecode::decoder::Decoder;
use oak_repl::{OakRepl, ReplHandler, HandleResult};

#[derive(Parser, Debug)]
#[command(name = "tsx", version = "0.1.0", author = "Nyar Project", about = "Mini TypeScript Executor (Simulating tsx)")]
struct Args {
    /// The input TypeScript file. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,
}

struct TsReplHandler {
    frontend: MiniTypescriptFrontend,
}

impl TsReplHandler {
    fn run_code_internal(frontend: &mut MiniTypescriptFrontend, source: &str) -> anyhow::Result<()> {
        match frontend.compile_to_nyar(source) {
            Ok(module) => {
                // 1. 获取主 Chunk (通常是第一个)
                if let Some(chunk) = module.chunks.first() {
                    // 2. 解码字节码为指令
                    let mut decoder = Decoder::new(&chunk.code);
                    let mut instructions = Vec::new();
                    while let Ok(ins) = decoder.next_result() {
                        instructions.push(ins);
                    }

                    // 3. 创建 VM 并执行
                    let mut vm = NyarVM::new(
                        module.constants.clone(),
                        module.chunks.clone(),
                        module.classes.clone(),
                        module.traits.clone(),
                        module.impls.clone(),
                        module.effects.clone(),
                    );
                    
                    match vm.execute(&instructions) {
                        Ok(val) => {
                            // Value 不实现 Debug，我们手动打印其基本信息
                            println!("Result Tag: {:?}", val.tag);
                        }
                        Err(e) => eprintln!("tsx: runtime error: {:?}", e),
                    }
                }
            }
            Err(e) => eprintln!("tsx: compilation error: {:?}", e),
        }
        Ok(())
    }
}

impl ReplHandler for TsReplHandler {
    fn prompt(&self, is_continuation: bool) -> &str {
        if is_continuation { "  ... " } else { "tsx> " }
    }

    fn is_complete(&self, code: &str) -> bool {
        if code.trim().is_empty() {
            return true;
        }
        
        let mut depth = 0;
        for c in code.chars() {
            match c {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }
        depth <= 0
    }

    fn handle_line(&mut self, line: &str) -> anyhow::Result<HandleResult> {
        let trimmed = line.trim();
        if trimmed == "exit()" || trimmed == "quit()" {
            return Ok(HandleResult::Exit);
        }
        let _ = Self::run_code_internal(&mut self.frontend, line);
        Ok(HandleResult::Continue)
    }

    fn get_indent(&self, code: &str) -> usize {
        let last_line = code.lines().last().unwrap_or("");
        let current_indent = last_line.len() - last_line.trim_start().len();
        if last_line.trim_end().ends_with('{') {
            current_indent + 2
        } else {
            current_indent
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut frontend = MiniTypescriptFrontend::new();

    if let Some(input_file) = args.input {
        let source_code = fs::read_to_string(&input_file)?;
        TsReplHandler::run_code_internal(&mut frontend, &source_code)?;
    } else {
        println!("Mini TypeScript REPL (Project Gaia)");
        println!("Type \"exit()\" or press Ctrl-D to exit.");
        
        let handler = TsReplHandler { frontend };
        let mut repl = OakRepl::new(handler);
        repl.run()?;
    }
    Ok(())
}
