use clap::Parser;
use mini_typescript::MiniTypescriptFrontend;
use nyar_vm::NyarDriver;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "tsx", version = "0.1.0", author = "Nyar Project", about = "Mini TypeScript Executor (Simulating tsx)")]
struct Args {
    /// The input TypeScript file or directory.
    #[arg(index = 1)]
    input: Option<PathBuf>,

    /// Enable verbose output (VM tracing).
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let input_path = match args.input {
        Some(path) => path,
        None => {
            println!("Usage: tsx <input_file>");
            return Ok(());
        }
    };

    let frontend = MiniTypescriptFrontend::new();
    let driver = NyarDriver::new();

    println!("tsx: Executing {:?}", input_path);
    driver.run_source(&frontend, &input_path)?;

    Ok(())
}
