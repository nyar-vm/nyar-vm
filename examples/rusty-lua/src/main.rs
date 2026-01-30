//! Mini Lua 语言编译器
//!
//! 这是一个类似 Lua 的语言前端演示程序，支持编译到 Nyar 字节码

use virtual_lua::MiniLuaFrontend;
use nyar_vm::NyarDriver;
use std::{fs, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file>", args[0]);
        exit(1);
    }

    let input_file = &args[1];

    // 读取输入文件
    let source_code = match fs::read_to_string(input_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("错误：无法读取文件 '{}': {}", input_file, e);
            exit(1);
        }
    };

    // 创建前端实例
    let frontend = MiniLuaFrontend::new();

    // 使用 NyarDriver 运行
    if let Err(e) = NyarDriver::run_source(&frontend, &source_code) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
