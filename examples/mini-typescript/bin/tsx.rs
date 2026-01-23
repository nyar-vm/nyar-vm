use clap::Parser;
use std::path::Path;
use mini_typescript::MiniTypescriptFrontend;
use mini_typescript::project::ProjectLoader;
use nyar_vm::NyarVM;
use oak_repl::{OakRepl, ReplHandler, HandleResult, ReplError};

#[derive(Parser, Debug)]
#[command(name = "tsx", version = "0.1.0", author = "Nyar Project", about = "Mini TypeScript Executor (Simulating tsx)")]
struct Args {
    /// The input TypeScript file or directory. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,
}

use std::fmt::{Display, Formatter};
use std::error::Error;

#[derive(Debug)]
pub enum TsError {
    Other(String),
}

impl Display for TsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TsError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for TsError {}

impl From<String> for TsError {
    fn from(s: String) -> Self {
        TsError::Other(s)
    }
}

impl From<&str> for TsError {
    fn from(s: &str) -> Self {
        TsError::Other(s.to_string())
    }
}

impl From<TsError> for ReplError {
    fn from(e: TsError) -> Self {
        ReplError::Other(e.to_string())
    }
}

impl From<ReplError> for TsError {
    fn from(e: ReplError) -> Self {
        TsError::Other(e.to_string())
    }
}

struct TsReplHandler {
    frontend: MiniTypescriptFrontend,
    vm: NyarVM,
}

impl TsReplHandler {
    fn new() -> Self {
        Self {
            frontend: MiniTypescriptFrontend::new(),
            vm: NyarVM::new(),
        }
    }

    fn run_project(&mut self, path: &str) -> Result<(), TsError> {
        let p = Path::new(path);
        let base_dir = if p.is_dir() { p } else { p.parent().unwrap_or(Path::new(".")) };
        let mut loader = ProjectLoader::new(base_dir);
        
        match loader.load_project(p) {
            Ok(modules) => {
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
                        println!("Execution finished. Result Tag: {:?}", val.tag);
                    }
                    Err(e) => eprintln!("tsx: runtime error: {:?}", e),
                }
            }
            Err(e) => eprintln!("tsx: project loading error: {}", e),
        }
        Ok(())
    }

    fn run_code_internal(&mut self, source: &str) -> Result<(), TsError> {
        match self.frontend.compile_to_nyar(source) {
            Ok(module) => {
                let module_idx = self.vm.load_module(module);
                match self.vm.execute(module_idx, 0) {
                    Ok(val) => {
                        println!("Result Tag: {:?}", val.tag);
                    }
                    Err(e) => eprintln!("tsx: runtime error: {:?}", e),
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

fn main() -> Result<(), TsError> {
    let args = Args::parse();
    let mut handler = TsReplHandler::new();

    if let Some(input_file) = args.input {
        handler.run_project(&input_file)?;
    } else {
        println!("Mini TypeScript REPL (Project Gaia)");
        println!("Type \"exit()\" or press Ctrl-D to exit.");
        
        let mut repl = OakRepl::new(handler);
        repl.run()?;
    }
    Ok(())
}
