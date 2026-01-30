//! Mini CSharp 语言编译器
//!
//! 这是一个类似 CSharp 的语言前端演示程序，支持编译到 Nyar 字节码

use mini_csharp::MiniCSharpFrontend;
use nyar_vm::NyarDriver;
use std::{fs, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file>", args[0]);
        exit(1);
    }

    let input_file = &args[1];
    let source_code = fs::read_to_string(input_file).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        exit(1);
    });

    let frontend = MiniCSharpFrontend::new();
    if let Err(e) = NyarDriver::run_source(&frontend, &source_code) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
