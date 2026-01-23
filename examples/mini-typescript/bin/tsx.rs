use clap::Parser;
use std::fs;
use mini_typescript::MiniTypescriptFrontend;
use gaia_jit::GaiaJit;
use oak_repl::{OakRepl, ReplHandler};

#[derive(Parser, Debug)]
#[command(name = "tsx", version = "0.1.0", author = "Gaia Project", about = "Mini TypeScript Executor (Simulating tsx)")]
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
        match frontend.compile_to_gaia(source) {
            Ok(module) => {
                let mut jit = GaiaJit::new();
                jit.load_module(module)?;
                jit.run("main")?;
            }
            Err(e) => eprintln!("tsx: compilation error: {:?}", e),
        }
        Ok(())
    }
}

impl ReplHandler for TsReplHandler {
    fn handle_line(&mut self, line: &str) -> anyhow::Result<bool> {
        if line == "exit()" || line == "quit()" {
            return Ok(false);
        }
        let _ = Self::run_code_internal(&mut self.frontend, line);
        Ok(true)
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
        let mut repl = OakRepl::new("tsx> ", handler);
        repl.run()?;
    }
    Ok(())
}
