use clap::Parser;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use valkyrie_language::compile_text_to_module;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input Valkyrie source file
    input: PathBuf,

    /// Output Nyar bytecode file
    output: PathBuf,
}

fn main() {
    let args = Args::parse();

    let src = fs::read_to_string(&args.input).expect("Failed to read input file");
    match compile_text_to_module(&src) {
        Ok(module) => {
            let bytes = module.encode();
            let mut file = fs::File::create(&args.output).expect("Failed to create output file");
            file.write_all(&bytes).expect("Failed to write output file");
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            std::process::exit(1);
        }
    }
}
