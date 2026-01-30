use clap::Parser;
use mini_typescript::MiniTypescriptFrontend;
use nyar_vm::NyarDriver;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "tsc",
    version = "0.1.0",
    author = "Nyar Project",
    about = "Mini TypeScript Compiler"
)]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: PathBuf,

    /// Output file
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.verbose {
        println!("Compiling {:?}...", args.input);
    }

    let frontend = MiniTypescriptFrontend::new();
    let driver = NyarDriver::new();

    if let Some(output_path) = args.output {
        #[cfg(feature = "native")]
        {
            println!("tsc: Compiling to {:?}", output_path);
            driver.compile_to_native(&frontend, &args.input, &output_path)?;
        }
        #[cfg(not(feature = "native"))]
        {
            println!("tsc: Native compilation is not supported in this build.");
            return Err("Native compilation is not supported in this build.".into());
        }
    } else {
        println!("tsc: Running {:?}", args.input);
        driver.run_source(&frontend, &args.input)?;
    }

    Ok(())
}
