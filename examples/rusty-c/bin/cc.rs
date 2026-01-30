//! Mini C 语言编译器
//!
//! 这是一个简单的 C 语言编译器，可以将 C 代码编译为 Gaia 指令

use clap::{Arg, ArgAction, Command};
use gaia_types::helpers::{AbiCompatible, ApiCompatible, Architecture, CompilationTarget};
use oak_core::{source::SourceText, Lexer};
use rusty_c::{CLexer, MiniCFrontend};
use std::{fs, path::Path};

fn main() {
    let matches = Command::new("cc")
        .version("0.1.0")
        .author("Nyar Project")
        .about("Mini C Compiler (Simulating cc/gcc)")
        .arg(
            Arg::new("input")
                .help("输入的 .vc 文件")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("target")
                .short('o')
                .long("target")
                .value_name("FILE")
                .help("输出文件路径"),
        )
        .arg(
            Arg::new("ast")
                .long("ast")
                .help("输出 AST")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("tokens")
                .long("tokens")
                .help("输出词法分析结果")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("gaia")
                .long("gaia")
                .short('g')
                .help("输出 Gaia 指令")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("binary")
                .long("binary")
                .short('b')
                .help("生成二进制可执行文件")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("以 JSON 格式输出 Gaia 指令")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let input_file = matches.get_one::<String>("input").unwrap();

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
        let config = oak_c::CLanguage::default();
        let lexer = CLexer::new(&config);
        let mut session = oak_core::parser::ParseSession::new(16);
        let source_text = SourceText::new(&source);
        let token_stream = lexer.lex(&source_text, &[], &mut session);
        println!("词法分析结果：");
        if let Ok(tokens) = token_stream.result {
            for token in tokens.iter() {
                println!("{:?}", token);
            }
        }
        return;
    }

    // 创建前端编译器
    let mut frontend = MiniCFrontend::new();

    // 如果需要 AST
    if matches.get_flag("ast") {
        let mut session = oak_core::parser::ParseSession::new(16);
        let source_text = SourceText::new(&source);
        match frontend.parse(&source_text, &mut session) {
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
        let compilation_target = CompilationTarget {
            build: Architecture::X86_64,
            host: AbiCompatible::PE,
            target: ApiCompatible::MicrosoftVisualC,
        };
        match frontend.compile_to_binary(&source, compilation_target) {
            Ok(binary) => {
                let input_path = Path::new(input_file);
                let output_path = input_path.with_extension("exe");
                std::fs::write(&output_path, binary).unwrap_or_else(|e| {
                    eprintln!("写入二进制文件失败：{}", e);
                    std::process::exit(1);
                });
                println!(
                    "使用 Gaia 生成的二进制文件已保存：{}",
                    output_path.display()
                );
            }
            Err(e) => {
                eprintln!("二进制生成错误：{:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 默认编译逻辑...
    println!("Simulating cc compilation for: {}", input_file);
}
