//! Mini Kotlin 语言编译器
//!
//! 这是一个类似 Kotlin 的语言前端演示程序，支持编译到 Nyar 字节码

use std::{path::Path, process::exit};
use mini_kotlin::MiniKotlinFrontend;
use nyar_vm::NyarDriver;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file>", args[0]);
        exit(1);
    }

    let input_file = Path::new(&args[1]);
    let frontend = MiniKotlinFrontend::new();
    let driver = NyarDriver::new();
    
    if let Err(e) = driver.run_source(&frontend, input_file) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
