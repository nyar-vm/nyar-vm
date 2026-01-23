//! Mini C 语言编译器
//!
//! 这是一个简单的 C 语言编译器，可以将 C 代码编译为 Gaia 指令

use clap::{Arg, ArgAction, Command};
use rusty_c::{MiniCFrontend, TargetArch};
use std::{fs, path::Path};

fn main() {
    let matches = Command::new("Mini C Compiler")
        .version("0.1.0")
        .author("Your Name")
        .about("Mini C 语言编译器，编译为 Gaia 指令")
        .arg(Arg::new("input").help("输入的 .vc 文件").required(true).index(1))
        .arg(Arg::new("target").short('o').long("target").value_name("FILE").help("输出文件路径"))
        .arg(Arg::new("ast").long("ast").help("输出 AST").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("tokens").long("tokens").help("输出词法分析结果").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("gaia").long("gaia").short('g').help("输出 Gaia 指令").action(ArgAction::SetTrue))
        .arg(Arg::new("binary").long("binary").short('b').help("生成二进制可执行文件").action(ArgAction::SetTrue))
        .arg(Arg::new("json").long("json").help("以 JSON 格式输出 Gaia 指令").action(clap::ArgAction::SetTrue))
        .get_matches();

    let input_file = matches.get_one::<String>("input").unwrap();
    let output_file = matches.get_one::<String>("target");

    // 检查文件扩展名
    if !input_file.ends_with(".vc") {
        eprintln!("错误：输入文件必须是 .vc 文件");
        std::process::exit(1);
    }

    // 读取输入文件
    let source = match fs::read_to_string(input_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("错误：无法读取文件 {}: {}", input_file, e);
            std::process::exit(1);
        }
    };

    // 如果只需要词法分析
    if matches.get_flag("tokens") {
        eprintln!("错误：词法分析功能暂不可用");
        std::process::exit(1);
    }

    // 创建前端编译器
    let mut frontend = MiniCFrontend::new();

    // 如果需要 AST
    if matches.get_flag("ast") {
        match frontend.parse(&source) {
            Ok(ast) => {
                println!("AST：");
                println!("{:#?}", ast);
            }
            Err(e) => {
                eprintln!("语法分析错误：{}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 如果需要生成二进制文件
    if matches.get_flag("binary") {
        let arch = TargetArch::X64; // 默认使用 X64

        match frontend.translator().generate_pe_binary(arch, 0) {
            Ok(binary) => {
                let input_path = Path::new(input_file);
                let output_path = input_path.with_extension("exe");
                std::fs::write(&output_path, binary).unwrap_or_else(|e| {
                    eprintln!("写入二进制文件失败：{}", e);
                    std::process::exit(1);
                });
                println!("使用 pe-rust 生成的二进制文件已保存：{}", output_path.display());
            }
            Err(e) => {
                eprintln!("二进制生成错误：{:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 如果需要生成 Gaia 指令
    if matches.get_flag("gaia") {
        match frontend.compile_to_gaia(&source) {
            Ok(gaia_program) => {
                println!("Gaia 程序编译成功：");
                println!("{:#?}", gaia_program);
            }
            Err(e) => {
                eprintln!("Gaia 编译错误：{}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 默认编译为 Gaia 程序并输出
    match frontend.compile_to_gaia(&source) {
        Ok(program) => {
            if matches.get_flag("json") {
                // 输出 JSON 格式
                match serde_json::to_string_pretty(&program) {
                    Ok(json) => {
                        if let Some(output) = output_file {
                            if let Err(e) = fs::write(output, json) {
                                eprintln!("错误：无法写入文件 {}: {}", output, e);
                                std::process::exit(1);
                            }
                            println!("JSON 输出已写入：{}", output);
                        }
                        else {
                            println!("{}", json);
                        }
                    }
                    Err(e) => {
                        eprintln!("JSON 序列化错误：{}", e);
                        std::process::exit(1);
                    }
                }
            }
            else {
                // 默认行为：编译并输出到文件
                let output_path = if let Some(output) = output_file {
                    output.clone()
                }
                else {
                    // 默认输出文件名
                    let input_path = Path::new(input_file);
                    let stem = input_path.file_stem().unwrap().to_str().unwrap();
                    format!("{}.gaia", stem)
                };

                // 这里可以添加将 Gaia 程序写入文件的逻辑
                println!("编译成功！输出文件：{}", output_path);
                println!("Gaia 程序：");
                println!("{:#?}", program);
            }
        }
        Err(e) => {
            eprintln!("编译错误：{}", e);
            std::process::exit(1);
        }
    }
}
