use clap::Parser;
use std::fs;
use mini_c::{frontend::MiniCFrontend, optimizer::MiniCOptimizer, runtime::MiniCRuntime};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

#[derive(Parser, Debug)]
#[command(name = "cling", version = "0.1.0", author = "Nyar Project", about = "Mini C Interpreter (Simulating Cling)")]
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
        let source = match fs::read_to_string(&input_file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("cling: error: could not read file '{}': {}", input_file, e);
                std::process::exit(1);
            }
        };
        run_code(&frontend, &optimizer, &mut runtime, &source, args.ast);
    } else {
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
            let optimized_uast = optimizer.optimize(uast);
            if let Err(e) = runtime.execute(optimized_uast) {
                eprintln!("cling: runtime error: {}", e);
            }
        }
        Err(e) => eprintln!("cling: error: {}", e),
    }
}

fn run_repl(frontend: &MiniCFrontend, optimizer: &MiniCOptimizer, runtime: &mut MiniCRuntime) {
    let mut rl = DefaultEditor::new().expect("Failed to create editor");
    println!("****************** CLING ******************");
    println!("* Interactive C Interpreter (Mini-C Mode) *");
    println!("*******************************************");

    loop {
        let readline = rl.readline("[cling]$ ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() { continue; }
                if line == ".q" || line == "exit" { break; }
                rl.add_history_entry(line).ok();
                run_code(frontend, optimizer, runtime, line, false);
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(err) => {
                println!("cling: error: {:?}", err);
                break;
            }
        }
    }
}
