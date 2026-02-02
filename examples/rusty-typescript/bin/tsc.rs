use clap::Parser;
use rusty_typescript::MiniTypescriptFrontend;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "tsc",
    version = "0.1.0",
    author = "Nyar Project",
    about = "Mini TypeScript AOT Compiler to WASM"
)]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: PathBuf,

    /// Output file (.wasm)
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Target architecture (e.g. wasm32-wasi)
    #[arg(short, long, default_value = "wasm32-wasi")]
    target: String,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let source = fs::read_to_string(&args.input)?;
    
    if args.verbose {
        println!("Compiling {:?} to WASM...", args.input);
    }

    let frontend = MiniTypescriptFrontend::new();
    
    // 调用 AOT 编译逻辑
    let wasm_bytes = frontend.compile_to_wasm(&source)?;

    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension("wasm");
        path
    });

    fs::write(&output_path, wasm_bytes)?;

    println!("Successfully compiled to {:?}", output_path);

    Ok(())
}
