use clap::Parser;
use oak_vfs::DiskVfs;
use rusty_typescript::RustyTypescriptFrontend;
use nyar_vm::NyarDriver;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "tsx",
    version = "0.1.0",
    author = "Nyar Project",
    about = "Mini TypeScript Executor (Simulating tsx)"
)]
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

    let frontend = RustyTypescriptFrontend::new();
    let driver = NyarDriver::new();
    let vfs = DiskVfs::new();

    println!("tsx: Executing {:?}", input_path);
    let uri = input_path.to_string_lossy();
    driver.run_source(&frontend, &vfs, &uri)?;

    Ok(())
}
