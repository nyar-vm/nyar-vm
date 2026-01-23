use clap::Parser;
use std::fs;
use mini_java::MiniJavaFrontend;

#[derive(Parser, Debug)]
#[command(name = "javac", version = "0.1.0", author = "Nyar Project", about = "Mini Java Compiler")]
struct Args {
    /// The input Java file
    #[arg(index = 1)]
    input: String,

    /// Output Nyar Binary file
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
    let frontend = MiniJavaFrontend::new();

    // Compile to Nyar instructions
    match frontend.compile_to_nyar(&source_code) {
        Ok(module) => {
            let data = module.encode();
            let out_path = args.output.unwrap_or_else(|| {
                let mut path = std::path::PathBuf::from(&args.input);
                path.set_extension("nb"); // Nyar Binary
                path.to_str().unwrap().to_string()
            });

            if let Err(e) = fs::write(&out_path, data) {
                eprintln!("Error: Could not write to output file '{}': {}", out_path, e);
                std::process::exit(1);
            }
            println!("Compiled successfully to '{}'", out_path);
        }
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
            std::process::exit(1);
        }
    }
}
