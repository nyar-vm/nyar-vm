use clap::Parser;
use rusty_c::frontend::RustyCFrontend;
use rusty_c::optimizer::RustyCOptimizer;
use rusty_c::runtime::RustyCRuntime;
use nyar_types::NyarError;
use nyar_vm::NyarDriver;
use oak_repl::{HandleResult, OakRepl, ReplError, ReplHandler};
use oak_vfs::{DiskVfs, Vfs};

#[derive(Parser, Debug)]
#[command(
    name = "cling",
    version = "0.1.0",
    author = "Gaia Project",
    about = "Rusty C Interpreter (Simulating Cling)"
)]
struct Args {
    /// The input C file. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,
}

use std::fmt;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum ClingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("REPL error: {0}")]
    Repl(#[from] ReplError),
    #[error("Nyar error: {0}")]
    Nyar(#[from] NyarError),
    #[error("{0}")]
    Other(String),
}

impl From<String> for ClingError {
    fn from(s: String) -> Self {
        ClingError::Other(s)
    }
}

impl From<&str> for ClingError {
    fn from(s: &str) -> Self {
        ClingError::Other(s.to_string())
    }
}

struct CReplHandler {
    frontend: RustyCFrontend,
    driver: NyarDriver,
    vfs: DiskVfs,
}

impl CReplHandler {
    fn new() -> Self {
        Self {
            frontend: RustyCFrontend::new(),
            driver: NyarDriver::new(),
            vfs: DiskVfs::new(),
        }
    }

    fn run_code_internal(&self, source: &str) -> Result<(), ClingError> {
        self.driver.run_code(&self.frontend, &self.vfs, source)?;
        Ok(())
    }
}

impl ReplHandler for CReplHandler {
    fn prompt(&self, is_continuation: bool) -> &str {
        if is_continuation {
            "  ... "
        } else {
            "[cling]$ "
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

        // C 语言通常需要分号结束语句，除非是块定义
        if depth <= 0 {
            let trimmed = code.trim_end();
            if trimmed.ends_with(';') || trimmed.ends_with('}') {
                return true;
            }
        }
        false
    }

    fn handle_line(&mut self, line: &str) -> Result<HandleResult, ReplError> {
        match self.run_code_internal(line) {
            Ok(_) => Ok(HandleResult::Continue),
            Err(e) => {
                eprintln!("cling error: {}", e);
                Ok(HandleResult::Continue)
            }
        }
    }

    fn get_indent(&self, code: &str) -> usize {
        let last_line = code.lines().last().unwrap_or("");
        let current_indent = last_line.len() - last_line.trim_start().len();
        if last_line.trim_end().ends_with('{') {
            current_indent + 4
        } else {
            current_indent
        }
    }
}

fn main() -> Result<(), ClingError> {
    let args = Args::parse();
    let handler = CReplHandler::new();

    if let Some(input_file) = args.input {
        let source_text = handler.vfs.get_source(&input_file)
            .ok_or_else(|| ClingError::Other(format!("File not found: {}", input_file)))?;
        let source = source_text.get_text_from(0);
        handler.run_code_internal(&source)?;
    } else {
        println!("Mini C REPL (Simulating Cling)");
        println!("Type 'exit' to quit.");
        let mut repl = OakRepl::new(handler);
        repl.run()?;
    }

    Ok(())
}
