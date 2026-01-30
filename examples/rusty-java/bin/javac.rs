use mini_java::MiniJavaFrontend;
use nyar_vm::NyarDriver;
use std::{path::Path, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: javac <input_file> [-o <output_file>]");
        exit(1);
    }

    let input_file = Path::new(&args[1]);
    let frontend = MiniJavaFrontend::new();
    let driver = NyarDriver::new();

    // 编译到 JVM .class
    let output_file = args
        .iter()
        .position(|a| a == "-o")
        .and_then(|i| args.get(i + 1))
        .map(Path::new)
        .unwrap_or(Path::new("Hello.class"));

    if let Err(e) = driver.compile_to_jvm(&frontend, input_file, output_file) {
        eprintln!("Compilation error: {:?}", e);
        exit(1);
    }
}
