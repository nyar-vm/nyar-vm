use clap::Parser;
use mini_typescript::errors::ScriptError;
use mini_typescript::MiniTypescriptFrontend;
use nyar_vm::bytecode::format::{EmitTarget, NyarModule};
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

fn main() -> Result<(), ScriptError> {
    let args = Args::parse();

    if args.verbose {
        println!("Compiling {:?}...", args.input);
    }

    // Read input file
    let source_code = fs::read_to_string(&args.input)
        .map_err(|e| ScriptError::io(e, args.input.clone()))?;

    // Create frontend instance
    let mut frontend = MiniTypescriptFrontend::new();

    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension(args.emit.extension());
        path
    });

    match args.emit {
        EmitTarget::Nyar => {
            let module = frontend.compile_to_nyar(&source_code)?;
            let data = module.encode();

            if args.run {
                run_module(&module)?;
            } else {
                fs::write(&output_path, data).map_err(|e| ScriptError::io(e, output_path.clone()))?;
                if args.verbose {
                    println!("Output written to {:?}", output_path);
                }
            }
        }
        EmitTarget::NyarToml => {
            let module = frontend.compile_to_nyar(&source_code)?;
            let toml = toml::to_string_pretty(&module).map_err(ScriptError::toml)?;
            fs::write(&output_path, toml).map_err(|e| ScriptError::io(e, output_path.clone()))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::NyarJson => {
            let module = frontend.compile_to_nyar(&source_code)
                .map_err(|e| ScriptError::from(format!("Compilation error: {:?}", e)))?;
            let json = serde_json::to_string_pretty(&module).map_err(|e| ScriptError::from(e.to_string()))?;
            fs::write(&output_path, json).map_err(|e| ScriptError::from(e.to_string()))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Wasm => {
            let wasm = frontend.compile_to_wasm(&source_code)
                .map_err(ScriptError::compile)?;
            fs::write(&output_path, wasm).map_err(|e| ScriptError::io(e, output_path.clone()))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Json => {
            let module = frontend.compile_to_nyar(&source_code)?;
            let json = serde_json::to_string_pretty(&module).map_err(ScriptError::json)?;
            fs::write(&output_path, json).map_err(|e| ScriptError::io(e, output_path.clone()))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Tokens => {
            let tokens = frontend.tokenize(&source_code)
                .map_err(ScriptError::compile)?;
            let mut output = String::new();
            for token in tokens {
                output.push_str(&format!("{:?}\n", token));
            }
            fs::write(&output_path, output).map_err(|e| ScriptError::io(e, output_path.clone()))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
        EmitTarget::Uir => {
            let (egraph, _root) = frontend.parse(&source_code)
                .map_err(ScriptError::compile)?;
            let output = format!("{:?}", egraph);
            fs::write(&output_path, output).map_err(|e| ScriptError::io(e, output_path.clone()))?;
            if args.verbose {
                println!("Output written to {:?}", output_path);
            }
        }
    }

    Ok(())
}

fn run_module(module: &nyar_vm::bytecode::format::NyarModule) -> Result<(), ScriptError> {
    let mut vm = NyarVM::new();

    vm.stdout = Some(Box::new(|msg: &str| {
        println!("{}", msg);
    }));

    let module_idx = vm.load_module(module.clone());
    let v = vm.execute(module_idx, 0)
        .map_err(ScriptError::vm)?;

    println!("Execution finished. Result Tag: {:?}", v.tag);
    Ok(())
}
