use clap::Parser;
use std::fs;
use mini_typescript::MiniTypescriptFrontend;
use gaia_jit::GaiaJit;

#[derive(Parser, Debug)]
#[command(name = "tsx", version = "0.1.0", author = "Gaia Project", about = "Mini TypeScript Executor")]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: String,
}

fn main() {
    let args = Args::parse();

    // Read input file
    let source_code = match fs::read_to_string(&args.input) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Could not read file '{}': {}", args.input, e);
            std::process::exit(1);
        }
    };

    // Create frontend instance
    let mut frontend = MiniTypescriptFrontend::new();

    // 1. Compile to Gaia instructions
    let module = match frontend.compile_to_gaia(&source_code) {
        Ok(module) => module,
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
            std::process::exit(1);
        }
    };

    // 2. Execute using Gaia JIT
    let mut jit = GaiaJit::new();
    if let Err(e) = jit.load_module(module) {
        eprintln!("JIT load error: {:?}", e);
        std::process::exit(1);
    }

    match jit.run("main") {
        Ok(_) => println!("\nExecution finished successfully."),
        Err(e) => {
            eprintln!("Execution error: {:?}", e);
            std::process::exit(1);
        }
    }
}
