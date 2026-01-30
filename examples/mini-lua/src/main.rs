//! Mini Lua 语言编译器
//!
//! 这是一个类似 Lua 的语言前端演示程序，支持编译到 Gaia 指令

use clap::{Arg, Command};
use std::{fs, path::Path};
use virtual_lua::MiniLuaFrontend;

fn main() {
    let matches = Command::new("Mini Lua 语言编译器")
        .version("1.0")
        .author("Gaia Project")
        .about("一个类似 Lua 的语言前端，支持编译到 Gaia 指令")
        .arg(Arg::new("input").help("输入的 Lua 源文件").required(true).index(1))
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
    let mut frontend = MiniLuaFrontend::new();

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
    if show_ast {
        match frontend.parse_to_ast(&source_code) {
            Ok(program) => {
                println!("=== 抽象语法树 ===");
                println!("{:#?}", program);
            }
            Err(e) => {
                eprintln!("解析错误: {:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if compile_gaia || compile_gaia_json {
        // 编译到 Gaia 指令
        match frontend.compile_to_gaia(&source_code) {
            Ok(module) => {
                if compile_gaia_json {
                    println!("{}", serde_json::to_string_pretty(&module).unwrap());
                } else {
                    println!("{:#?}", module);
                }

                if let Some(out_path) = output_file {
                    let data = if out_path.ends_with(".json") {
                        serde_json::to_string_pretty(&module).unwrap().into_bytes()
                    } else {
                        // 暂时使用调试输出
                        format!("{:#?}", module).into_bytes()
                    };
                    if let Err(e) = fs::write(out_path, data) {
                        eprintln!("错误：无法写入文件 '{}': {}", out_path, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("编译错误: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 默认行为：显示解析成功信息
    match frontend.parse_to_ast(&source_code) {
        Ok(program) => {
            println!("✅ 解析成功！");
            println!("文件: {}", input_file);
            println!("顶级语句数量: {}", program.statements.len());
        }
        Err(e) => {
            eprintln!("解析错误: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("\n💡 提示：");
    println!("  使用 --ast 查看抽象语法树");
    println!("  使用 --tokens 查看词法分析结果");
    println!("  使用 --gaia 编译到 Gaia 指令");
    println!("  使用 --gaia-json 编译到 Gaia 指令 (JSON 格式)");
}
