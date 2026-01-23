use clap::Parser;
use std::fs;
use std::path::Path;
use virtual_python::MiniPythonFrontend;
use gaia_jit::GaiaJit;

#[derive(Parser, Debug)]
#[command(name = "python", version = "1.0", author = "Gaia Project", about = "Mini Python Interpreter and Compiler")]
struct Args {
    /// The input Python source file (.py)
    #[arg(index = 1)]
    input: String,

    /// Output file path
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Compile to Python bytecode (.pyc) instead of executing
    #[arg(long)]
    compile: bool,

    /// Output Gaia instructions as JSON
    #[arg(long)]
    gaia_json: bool,

    /// Show Abstract Syntax Tree
    #[arg(long)]
    ast: bool,

    /// Show Tokens
    #[arg(long)]
    tokens: bool,
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

    let mut frontend = MiniPythonFrontend::new();

    // 1. Handle tokens/ast inspection
    if args.tokens {
        match frontend.tokenize(&source_code) {
            Ok(tokens) => {
                println!("=== Tokens ===");
                for token in tokens {
                    println!("{:?}", token);
                }
            }
            Err(e) => {
                eprintln!("Lexer error: {:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if args.ast {
        match frontend.parse(&source_code) {
            Ok(program) => {
                println!("=== AST ===");
                println!("{:#?}", program);
            }
            Err(e) => {
                eprintln!("Parser error: {:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 2. Handle compilation to .pyc
    if args.compile {
        match frontend.compile_to_pyc(&source_code, &args.input) {
            Ok(pyc_data) => {
                let out_path = args.output.unwrap_or_else(|| {
                    let path = Path::new(&args.input);
                    path.with_extension("pyc").to_string_lossy().into_owned()
                });
                if let Err(e) = fs::write(&out_path, pyc_data) {
                    eprintln!("Error: Could not write to output file '{}': {}", out_path, e);
                    std::process::exit(1);
                }
                println!("Successfully compiled to '{}'", out_path);
            }
            Err(e) => {
                eprintln!("Compilation error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 3. Handle Gaia JSON output
    if args.gaia_json {
        match frontend.compile_to_gaia(&source_code) {
            Ok(module) => {
                let json = serde_json::to_string_pretty(&module).unwrap();
                if let Some(out_path) = args.output {
                    if let Err(e) = fs::write(out_path, json) {
                        eprintln!("Error: Could not write to output file: {}", e);
                        std::process::exit(1);
                    }
                } else {
                    println!("{}", json);
                }
            }
            Err(e) => {
                eprintln!("Gaia compilation error: {:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 4. Default: Execute using Gaia JIT (like real python command runs code)
    match frontend.compile_to_gaia(&source_code) {
        Ok(module) => {
            let mut jit = GaiaJit::new();
            if let Err(e) = jit.load_module(module) {
                eprintln!("JIT load error: {:?}", e);
                std::process::exit(1);
            }

            match jit.run("main") {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Execution error: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
            std::process::exit(1);
        }
    }
}
