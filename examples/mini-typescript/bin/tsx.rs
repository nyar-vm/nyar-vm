use clap::Parser;
use std::fs;
use mini_typescript::MiniTypescriptFrontend;
use gaia_jit::GaiaJit;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

#[derive(Parser, Debug)]
#[command(name = "tsx", version = "0.1.0", author = "Gaia Project", about = "Mini TypeScript Executor (Simulating tsx)")]
struct Args {
    /// The input TypeScript file. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,
}

fn main() {
    let args = Args::parse();

    let mut frontend = MiniTypescriptFrontend::new();

    if let Some(input_file) = args.input {
        // 1. File execution mode
        let source_code = match fs::read_to_string(&input_file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error: Could not read file '{}': {}", input_file, e);
                std::process::exit(1);
            }
        };

        run_code(&mut frontend, &source_code);
    } else {
        // 2. REPL mode
        run_repl(&mut frontend);
    }
}

fn run_code(frontend: &mut MiniTypescriptFrontend, source: &str) {
    // 1. Compile to Gaia instructions
    match frontend.compile_to_gaia(source) {
        Ok(module) => {
            // 2. Execute using Gaia JIT
            let mut jit = GaiaJit::new();
            if let Err(e) = jit.load_module(module) {
                eprintln!("JIT load error: {:?}", e);
                return;
            }

            match jit.run("main") {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Execution error: {:?}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
        }
    }
}

fn run_repl(frontend: &mut MiniTypescriptFrontend) {
    let mut rl = DefaultEditor::new().expect("Failed to create editor");
    println!("Mini TypeScript REPL (Project Gaia)");
    println!("Type \"exit()\" or press Ctrl-D to exit.");

    loop {
        let readline = rl.readline("tsx> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit()" || line == "quit()" {
                    break;
                }
                rl.add_history_entry(line).ok();
                
                run_code(frontend, line);
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("EOF");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
}
