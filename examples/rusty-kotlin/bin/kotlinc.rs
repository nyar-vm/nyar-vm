use std::{path::Path, process::exit};
use mini_kotlin::MiniKotlinFrontend;
use nyar_vm::NyarDriver;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: kotlinc <input_file> [-o <output_file>]");
        exit(1);
    }

    let input_file = Path::new(&args[1]);
    let frontend = MiniKotlinFrontend::new();
    let driver = NyarDriver::new();
    
    // 模拟编译：目前先编译为 native (stub)
    let output_file = args.iter().position(|a| a == "-o")
        .and_then(|i| args.get(i + 1))
        .map(Path::new)
        .unwrap_or(Path::new("out.exe"));

    if let Err(e) = driver.compile_to_native(&frontend, input_file, output_file) {
        eprintln!("Compilation error: {:?}", e);
        exit(1);
    }
}
