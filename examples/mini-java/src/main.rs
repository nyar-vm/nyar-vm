//! Mini Java 语言编译器
//!
//! 这是一个类似 Java 的语言前端演示程序，支持编译到 Nyar 字节码

use clap::{Arg, Command};
use std::fs;
use mini_java::MiniJavaFrontend;

fn main() {
    let matches = Command::new("Mini Java 语言编译器")
        .version("1.0")
        .author("Nyar Project")
        .about("一个类似 Java 的语言前端，支持编译到 Nyar 字节码")
        .arg(Arg::new("input").help("输入的 Java 源文件").required(true).index(1))
        .arg(Arg::new("output").short('o').long("output").value_name("FILE").help("输出文件路径").required(false))
        .arg(Arg::new("ast").long("ast").help("只输出抽象语法树").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("nyar").long("nyar").help("编译到 Nyar 字节码并输出").action(clap::ArgAction::SetTrue))
        .get_matches();

    let input_file = matches.get_one::<String>("input").unwrap();
    let output_file = matches.get_one::<String>("output");
    let show_ast = matches.get_flag("ast");
    let compile_nyar = matches.get_flag("nyar");

    // 读取输入文件
    let source_code = match fs::read_to_string(input_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("错误：无法读取文件 '{}': {}", input_file, e);
            std::process::exit(1);
        }
    };

    // 创建前端实例
    let frontend = MiniJavaFrontend::new();

    // 解析源代码
    let program = match frontend.parse(&source_code) {
        Ok(program) => program,
        Err(e) => {
            eprintln!("解析错误: {:?}", e);
            std::process::exit(1);
        }
    };

    if show_ast {
        // 只显示 AST
        println!("=== 抽象语法树 ===");
        println!("{:#?}", program);
        return;
    }

    if compile_nyar {
        // 编译到 Nyar 字节码
        match frontend.compile_to_nyar(&source_code) {
            Ok(module) => {
                println!("=== Nyar 模块 ===");
                println!("{:#?}", module);
                if let Some(out_path) = output_file {
                    let data = module.encode();
                    if let Err(e) = fs::write(out_path, data) {
                        eprintln!("错误：无法写入输出文件 '{}': {}", out_path, e);
                        std::process::exit(1);
                    }
                    println!("已导出到二进制文件 '{}'", out_path);
                }
            }
            Err(e) => {
                eprintln!("编译 Nyar 错误: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}
