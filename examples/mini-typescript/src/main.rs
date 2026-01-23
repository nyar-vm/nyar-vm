//! Mini TypeScript 语言编译器
//!
//! 这是一个类似 TypeScript 的语言前端演示程序，支持编译到 Gaia 指令
//! 遵循 Project Chomsky Whitebook 规范，使用 Oaks 进行文本解析，Chomsky 进行优化，Gaia 进行二进制生成。

use clap::{Arg, Command};
use std::{fs, path::Path};
use mini_typescript::MiniTypescriptFrontend;

fn main() {
    let matches = Command::new("Mini TypeScript 语言编译器")
        .version("1.0")
        .author("Gaia Project")
        .about("一个类似 TypeScript 的语言前端，支持编译到 Gaia 指令")
        .arg(Arg::new("input").help("输入的 TypeScript 源文件").required(true).index(1))
        .arg(Arg::new("output").short('o').long("output").value_name("FILE").help("输出文件路径").required(false))
        .arg(Arg::new("ast").long("ast").help("只输出抽象语法树").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("tokens").long("tokens").help("只输出词法分析结果").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("gaia").long("gaia").help("编译到 Gaia 指令并输出").action(clap::ArgAction::SetTrue))
        .arg(
            Arg::new("gaia-json").long("gaia-json").help("编译到 Gaia 指令并输出为 JSON 格式").action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let input_file = matches.get_one::<String>("input").unwrap();
    let output_file = matches.get_one::<String>("output");
    let show_ast = matches.get_flag("ast");
    let show_tokens = matches.get_flag("tokens");
    let compile_gaia = matches.get_flag("gaia");
    let compile_gaia_json = matches.get_flag("gaia-json");

    // 读取输入文件
    let source_code = match fs::read_to_string(input_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("错误：无法读取文件 '{}': {}", input_file, e);
            std::process::exit(1);
        }
    };

    // 创建前端实例
    let mut frontend = MiniTypescriptFrontend::new();

    if show_tokens {
        // 只显示词法分析结果
        match frontend.tokenize(&source_code) {
            Ok(tokens) => {
                println!("=== 词法分析结果 ===");
                for token in tokens.iter() {
                    println!("{:?}", token);
                }
            }
            Err(e) => {
                eprintln!("词法分析错误: {:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

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

    if compile_gaia || compile_gaia_json {
        // 编译到 Gaia 指令
        match frontend.compile_to_gaia(&source_code) {
            Ok(module) => {
                if compile_gaia_json {
                    let json = serde_json::to_string_pretty(&module).unwrap();
                    if let Some(out_path) = output_file {
                        fs::write(out_path, json).unwrap();
                    } else {
                        println!("{}", json);
                    }
                } else {
                    println!("=== Gaia 模块 ===");
                    println!("{:#?}", module);
                    if let Some(out_path) = output_file {
                        // 这里可以调用 Gaia 的二进制生成功能
                        println!("警告：尚未实现直接导出到二进制文件 '{}'", out_path);
                    }
                }
            }
            Err(e) => {
                eprintln!("编译 Gaia 错误: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}
