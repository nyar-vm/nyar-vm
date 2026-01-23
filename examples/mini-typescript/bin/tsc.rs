use clap::Parser;
use std::fs;
use mini_typescript::MiniTypescriptFrontend;

#[derive(Parser, Debug)]
#[command(name = "tsc", version = "0.1.0", author = "Gaia Project", about = "Mini TypeScript Compiler")]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: String,

    /// Output Gaia JSON file
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,
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

    // Compile to Gaia instructions
    match frontend.compile_to_gaia(&source_code) {
        Ok(module) => {
            let json = serde_json::to_string_pretty(&module).unwrap();
            if let Some(out_path) = args.output {
                if let Err(e) = fs::write(&out_path, json) {
                    eprintln!("Error: Could not write to output file '{}': {}", out_path, e);
                    std::process::exit(1);
                }
                println!("Compiled successfully to '{}'", out_path);
            } else {
                println!("{}", json);
            }
        }
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
            std::process::exit(1);
        }
    }
}
