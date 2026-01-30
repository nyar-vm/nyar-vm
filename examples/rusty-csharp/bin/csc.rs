use clap::Parser;
use std::fs;
use std::path::Path;
use mini_csharp::MiniCSharpFrontend;
use chomsky_full::extract::Backend;

#[derive(Parser, Debug)]
#[command(name = "csc", version = "0.1.0", author = "Nyar Project", about = "Mini CSharp Compiler")]
struct Args {
    /// The input CSharp file
    #[arg(index = 1)]
    input: String,

    /// The output Nyar Binary file
    #[arg(short, long, default_value = "out.nyar")]
    output: String,
}

fn main() {
    let args = Args::parse();

    // Read input file
    let source = match fs::read_to_string(&args.input) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Could not read file '{}': {}", args.input, e);
            std::process::exit(1);
        }
    };

    // Initialize frontend
    let frontend = MiniCSharpFrontend::new();

    // Compile
    println!("Compiling {}...", args.input);
    let artifact = match frontend.generate_from_source(&source) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: Compilation failed: {:?}", e);
            std::process::exit(1);
        }
    };

    // Save output
    let data = match artifact {
        chomsky_full::extract::BackendArtifact::Binary(data) => data,
        chomsky_full::extract::BackendArtifact::Source(s) => s.into_bytes(),
    };

    match fs::write(&args.output, data) {
        Ok(_) => println!("Successfully compiled to {}", args.output),
        Err(e) => {
            eprintln!("Error: Could not write output file '{}': {}", args.output, e);
            std::process::exit(1);
        }
    }
}
