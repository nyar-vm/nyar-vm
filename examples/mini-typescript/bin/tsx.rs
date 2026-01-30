use clap::Parser;
use mini_typescript::errors::ScriptError;
use mini_typescript::project::ProjectLoader;
use mini_typescript::MiniTypescriptFrontend;
use nyar_vm::NyarVM;
use oak_repl::{HandleResult, OakRepl, ReplError, ReplHandler};
use std::path::Path;

#[derive(Parser, Debug, Clone)]
#[command(name = "tsx", version = "0.1.0", author = "Nyar Project", about = "Mini TypeScript Executor (Simulating tsx)")]
struct Args {
    /// The input TypeScript file or directory. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,

    /// Compile to WASM instead of running in the VM.
    #[arg(long)]
    wasm: bool,

    /// Enable verbose output (VM tracing).
    #[arg(short, long)]
    verbose: bool,
}

impl From<ScriptError> for ReplError {
    fn from(e: ScriptError) -> Self {
        ReplError::Other(e.to_string())
    }
}

impl From<ReplError> for ScriptError {
    fn from(e: ReplError) -> Self {
        ScriptError::from(e.to_string())
    }
}

struct TsReplHandler {
    frontend: MiniTypescriptFrontend,
    vm: NyarVM,
    args: Args,
}

impl TsReplHandler {
    fn new(args: Args) -> Self {
        Self { frontend: MiniTypescriptFrontend::new(), vm: NyarVM::new(), args }
    }

    fn run_project(&mut self, path: &str) -> Result<(), ScriptError> {
        let p = Path::new(path);
        let base_dir = if p.is_dir() { p } else { p.parent().unwrap_or(Path::new(".")) };
        let mut loader = ProjectLoader::new(base_dir);

        match loader.load_project(p) {
            Ok(modules) => {
                if self.args.wasm {
                    println!("tsx: WASM compilation is not yet fully implemented for projects.");
                    return Ok(());
                }

                let mut entry_module_idx = 0;
                for (i, module) in modules.into_iter().enumerate() {
                    let idx = self.vm.load_module(module);
                    if i == 0 {
                        entry_module_idx = idx;
                    }
                }

                // Execute main chunk of the entry module (usually index 0)
                match self.vm.execute(entry_module_idx, 0) {
                    Ok(val) => {
                        if self.args.verbose {
                            println!("Execution finished. Result: {} (Tag: {:?})", val, val.tag);
                        } else {
                            println!("{}", val);
                        }
                    }
                    Err(e) => {
                        eprintln!("tsx: runtime error: {:?}", e);
                        self.vm.print_traceback(&e);
                    }
                }
            }
            Err(e) => eprintln!("tsx: project loading error: {}", e),
        }
        Ok(())
    }

    fn run_code_internal(&mut self, source: &str) -> Result<(), ScriptError> {
        if self.args.wasm {
            match self.frontend.compile_to_wasm(source) {
                Ok(wasm) => {
                    println!("tsx: compiled to WASM ({} bytes)", wasm.len());
                }
                Err(e) => eprintln!("tsx: WASM compilation error: {:?}", e),
            }
            return Ok(());
        }

        match self.frontend.compile_to_nyar(source) {
            Ok(module) => {
                let module_idx = self.vm.load_module(module);
                match self.vm.execute(module_idx, 0) {
                    Ok(val) => {
                        if self.args.verbose {
                            println!("Result: {} (Tag: {:?})", val, val.tag);
                        } else {
                            println!("{}", val);
                        }
                    }
                    Err(e) => {
                        eprintln!("tsx: runtime error: {:?}", e);
                        self.vm.print_traceback(&e);
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
        if is_continuation {
            "  ... "
        } else {
            "tsx> "
        }
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

    fn handle_line(&mut self, line: &str) -> Result<HandleResult, ReplError> {
        let trimmed = line.trim();
        if trimmed == "exit()" || trimmed == "quit()" {
            return Ok(HandleResult::Exit);
        }
        let _ = self.run_code_internal(line);
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

fn main() -> Result<(), ScriptError> {
    let args = Args::parse();
    let args_for_handler = args.clone();
    let mut handler = TsReplHandler::new(args_for_handler);

    if let Some(input_file) = &args.input {
        handler.run_project(input_file)?;
    } else {
        println!("Mini TypeScript REPL (Project Gaia)");
        println!("Type \"exit()\" or press Ctrl-D to exit.");

        let mut repl = OakRepl::new(handler);
        repl.run()?;
    }
    Ok(())
}
