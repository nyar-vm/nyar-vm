use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use mini_typescript::MiniTypescriptFrontend;
use nyar_vm::vm::interpreter::NyarVM;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "tsc", version = "0.1.0", author = "Nyar Project", about = "Mini TypeScript Compiler")]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: PathBuf,

    /// Output file
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// What to emit
    #[arg(short, long, value_enum, default_value_t = EmitTarget::Nyar)]
    emit: EmitTarget,

    /// Run the compiled program immediately
    #[arg(short, long)]
    run: bool,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum EmitTarget {
    /// Nyar Binary module (.nyar)
    Nyar,
    /// Nyar TOML module (.nyar.toml)
    NyarToml,
    /// WASM component (.wasm)
    Wasm,
    /// UIR as JSON
    Json,
    /// Tokens
    Tokens,
    /// UIR EGraph debug output
    Uir,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.verbose {
        println!("Compiling {:?}...", args.input);
    }

    // Read input file
    let source_code = fs::read_to_string(&args.input)
        .with_context(|| format!("Could not read file '{:?}'", args.input))?;

    // Create frontend instance
    let mut frontend = MiniTypescriptFrontend::new();

    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut path = args.input.clone();
        match args.emit {
            EmitTarget::Nyar => path.set_extension("nyar"),
            EmitTarget::NyarToml => path.set_extension("nyar.toml"),
            EmitTarget::Wasm => path.set_extension("wasm"),
            EmitTarget::Json => path.set_extension("json"),
            EmitTarget::Tokens => path.set_extension("tokens"),
            EmitTarget::Uir => path.set_extension("uir"),
        };
        path
    });

    match args.emit {
        EmitTarget::Nyar => {
            let module = frontend.compile_to_nyar(&source_code)
                .map_err(|e| anyhow::anyhow!("Compilation error: {:?}", e))?;
            let data = module.encode();

            if args.run {
                run_module(&module)?;
            } else {
                fs::write(&output_path, data).with_context(|| format!("Failed to write output to {:?}", output_path))?;
                if args.verbose {
                    println!("Output written to {:?}", output_path);
                }
            }
        }
        EmitTarget::NyarToml => {
            let module = frontend.compile_to_nyar(&source_code)
                .map_err(|e| anyhow::anyhow!("Compilation error: {:?}", e))?;
            let toml = toml::to_string_pretty(&module).context("Failed to serialize to TOML")?;
            fs::write(&output_path, toml).with_context(|| format!("Failed to write output to {:?}", output_path))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Wasm => {
            let wasm = frontend.compile_to_wasm(&source_code)
                .map_err(|e| anyhow::anyhow!("WASM Compilation error: {}", e))?;
            fs::write(&output_path, wasm).with_context(|| format!("Failed to write output to {:?}", output_path))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Json => {
            let module = frontend.compile_to_nyar(&source_code)
                .map_err(|e| anyhow::anyhow!("Compilation error: {:?}", e))?;
            let json = serde_json::to_string_pretty(&module).context("Failed to serialize to JSON")?;
            fs::write(&output_path, json).with_context(|| format!("Failed to write output to {:?}", output_path))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Tokens => {
            let tokens = frontend.tokenize(&source_code)
                .map_err(|e| anyhow::anyhow!("Tokenization error: {}", e))?;
            let mut output = String::new();
            for token in tokens {
                output.push_str(&format!("{:?}\n", token));
            }
            fs::write(&output_path, output).with_context(|| format!("Failed to write output to {:?}", output_path))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Uir => {
            let (egraph, _root) = frontend.parse(&source_code)
                .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;
            let output = format!("{:?}", egraph);
            fs::write(&output_path, output).with_context(|| format!("Failed to write output to {:?}", output_path))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
    }

    Ok(())
}

fn run_module(module: &nyar_vm::bytecode::format::NyarModule) -> Result<()> {
    let mut vm = NyarVM::new();

    vm.stdout = Some(Box::new(|msg: &str| {
        println!("{}", msg);
    }));

    let module_idx = vm.load_module(module.clone());
    let v = vm.execute(module_idx, 0)
        .map_err(|e| anyhow::anyhow!("Runtime error: {:?}", e))?;

    println!("Execution finished. Result Tag: {:?}", v.tag);
    Ok(())
}
