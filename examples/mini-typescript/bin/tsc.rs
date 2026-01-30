use clap::Parser;
use std::fs;
use mini_typescript::MiniTypescriptFrontend;

#[derive(Parser, Debug)]
#[command(name = "tsc", version = "0.1.0", author = "Nyar Project", about = "Mini TypeScript Compiler")]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: String,

    /// Output Nyar Binary file
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Compile to WASM component
    #[arg(long)]
    wasm: bool,
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

    if args.wasm {
        match frontend.compile_to_wasm(&source_code) {
            Ok(wasm) => {
                if let Some(out_path) = args.output {
                    if let Err(e) = fs::write(&out_path, wasm) {
                        eprintln!("Error: Could not write to output file '{}': {}", out_path, e);
                        std::process::exit(1);
                    }
                    println!("Compiled successfully to WASM '{}'", out_path);
                } else {
                    println!("Compiled successfully to WASM (size: {} bytes)", wasm.len());
                }
            }
            Err(e) => {
                eprintln!("WASM Compilation error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Compile to Nyar instructions
    match frontend.compile_to_nyar(&source_code) {
        Ok(module) => {
            let data = module.encode();
            if let Some(out_path) = args.output {
                if let Err(e) = fs::write(&out_path, data) {
                    eprintln!("Error: Could not write to output file '{}': {}", out_path, e);
                    std::process::exit(1);
                }
                println!("Compiled successfully to '{}'", out_path);
            } else {
                // 如果没有指定输出文件，可以尝试反序列化为 JSON 打印或直接输出 16 进制
                let json = serde_json::to_string_pretty(&module).unwrap();
                println!("{}", json);
            }
        }
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
            std::process::exit(1);
        }
    }
}
