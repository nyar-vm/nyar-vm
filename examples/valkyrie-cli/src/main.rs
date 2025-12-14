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

    fn collect_vk_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            if let Ok(read_dir) = fs::read_dir(&d) {
                for entry in read_dir.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().map_or(false, |ext| ext == "vk") {
                        out.push(p);
                    }
                }
            }
        }
        out
    }

    let src = if args.input.is_dir() {
        let mut files: Vec<PathBuf> = collect_vk_files(&args.input);
        files.sort_by(|a, b| {
            let priority = |p: &PathBuf| {
                let name = p.file_name().unwrap().to_str().unwrap();
                if name.contains("lexer") { 0 }
                else if name.contains("ast") { 1 }
                else if name.contains("parser") { 2 }
                else if name.contains("hir") { 3 }
                else if name.contains("mir") { 4 }
                else if name.contains("compiler") { 5 }
                else if name.contains("main") { 100 }
                else { 50 }
            };
            let pa = priority(a);
            let pb = priority(b);
            if pa != pb {
                pa.cmp(&pb)
            } else {
                a.cmp(b)
            }
        });
        println!("Compiling files in order: {:?}", files);
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
