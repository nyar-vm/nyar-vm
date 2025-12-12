use clap::Parser;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
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

    let src = if args.input.is_dir() {
        let mut files: Vec<PathBuf> = fs::read_dir(&args.input)
            .expect("Failed to read input directory")
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().map_or(false, |ext| ext == "vk"))
            .collect();
        files.sort();
        let mut buf = String::new();
        for p in files {
            let mut f = fs::File::open(&p).expect("Failed to open source file");
            let mut s = String::new();
            f.read_to_string(&mut s)
                .expect("Failed to read source file");
            buf.push_str(&s);
            buf.push_str("\n");
        }
        buf
    } else {
        fs::read_to_string(&args.input).expect("Failed to read input file")
    };
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
