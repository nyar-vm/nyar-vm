use clap::Parser;
use std::fs;
use mini_c::{frontend::MiniCFrontend, optimizer::MiniCOptimizer, runtime::MiniCRuntime};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

#[derive(Parser, Debug)]
#[command(name = "cx", version = "0.1.0", author = "Nyar Project", about = "Mini C Executor & REPL (Simulating tsx/pyx pattern)")]
struct Args {
    /// Input C source file. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,

    /// Show Abstract Syntax Tree
    #[arg(long)]
    ast: bool,
}

fn main() {
    let args = Args::parse();

    let frontend = MiniCFrontend::new();
    let optimizer = MiniCOptimizer::new();
    let mut runtime = MiniCRuntime::new();

    if let Some(input_file) = args.input {
        // 1. File execution mode
        let source = match fs::read_to_string(&input_file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("cx: error: could not read file '{}': {}", input_file, e);
                std::process::exit(1);
            }
        };

        run_code(&frontend, &optimizer, &mut runtime, &source, args.ast);
    } else {
        // 2. REPL mode
        run_repl(&frontend, &optimizer, &mut runtime);
    }
}

fn run_code(frontend: &MiniCFrontend, optimizer: &MiniCOptimizer, runtime: &mut MiniCRuntime, source: &str, show_ast: bool) {
    match frontend.parse(source) {
        Ok(uast) => {
            if show_ast {
                println!("{:#?}", uast);
                return;
            }

            // 2. Optimizer: Optimize UAST
            let optimized_uast = optimizer.optimize(uast);

            // 3. Runtime: Execute
            if let Err(e) = runtime.execute(optimized_uast) {
                eprintln!("cx: runtime error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("cx: error: {}", e);
        }
    }
}

fn run_repl(frontend: &MiniCFrontend, optimizer: &MiniCOptimizer, runtime: &mut MiniCRuntime) {
    let mut rl = DefaultEditor::new().expect("Failed to create editor");
    println!("Mini C REPL (cx)");
    println!("Type \"exit\" or press Ctrl-D to exit.");

    loop {
        let readline = rl.readline("cx> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    break;
                }
                rl.add_history_entry(line).ok();
                
                run_code(frontend, optimizer, runtime, line, false);
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
                println!("cx: error: {:?}", err);
                break;
            }
        }
    }
}
