//! Mini Python 语言编译器
//!
//! 这是一个类似 Python 的语言前端演示程序，支持编译到 Gaia 指令或 Python 字节码 (.pyc)

use clap::{Arg, Command};
use std::{fs, path::Path};
use virtual_python::MiniPythonFrontend;

fn main() {
    let matches = Command::new("Mini Python 语言编译器")
        .version("1.0")
        .author("Gaia Project")
        .about("一个类似 Python 的语言前端，支持编译到 Gaia 指令或 Python 字节码 (.pyc)")
        .arg(Arg::new("input").help("输入的 Python 源文件").required(true).index(1))
        .arg(Arg::new("output").short('o').long("output").value_name("FILE").help("输出文件路径").required(false))
        .arg(Arg::new("ast").long("ast").help("只输出抽象语法树").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("tokens").long("tokens").help("只输出词法分析结果").action(clap::ArgAction::SetTrue))
        .arg(Arg::new("gaia").long("gaia").help("编译到 Gaia 指令并输出").action(clap::ArgAction::SetTrue))
        .arg(
            Arg::new("gaia-json").long("gaia-json").help("编译到 Gaia 指令并输出为 JSON 格式").action(clap::ArgAction::SetTrue),
        )
        .arg(Arg::new("pyc").long("pyc").help("编译到 Python 字节码 (.pyc)").action(clap::ArgAction::SetTrue))
        .get_matches();

    let input_file = matches.get_one::<String>("input").unwrap();
    let output_file = matches.get_one::<String>("output");
    let show_ast = matches.get_flag("ast");
    let show_tokens = matches.get_flag("tokens");
    let compile_gaia = matches.get_flag("gaia");
    let compile_gaia_json = matches.get_flag("gaia-json");
    let compile_pyc = matches.get_flag("pyc");

    // 读取输入文件
    let source_code = match fs::read_to_string(input_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("错误：无法读取文件 '{}': {}", input_file, e);
            std::process::exit(1);
        }
    };

    // 创建前端实例
    let mut frontend = MiniPythonFrontend::new();

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

    if compile_pyc {
        match frontend.compile_to_pyc(&source_code, input_file) {
            Ok(pyc_data) => {
                let out_path = output_file.cloned().unwrap_or_else(|| {
                    let path = Path::new(input_file);
                    path.with_extension("pyc").to_string_lossy().into_owned()
                });
                if let Err(e) = fs::write(&out_path, pyc_data) {
                    eprintln!("错误：无法写入文件 '{}': {}", out_path, e);
                    std::process::exit(1);
                }
                println!("成功编译到 {}", out_path);
            }
            Err(e) => {
                eprintln!("编译 PYC 错误: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if compile_gaia || compile_gaia_json {
        // 编译到 Gaia 指令
        match frontend.compile_to_gaia(&source_code) {
            Ok(gaia_program) => {
                if compile_gaia_json {
                    // 输出为 JSON 格式
                    match serde_json::to_string_pretty(&gaia_program) {
                        Ok(json) => {
                            if let Some(output_path) = output_file {
                                match fs::write(output_path, json) {
                                    Ok(_) => println!("✅ Gaia JSON 已写入: {}", output_path),
                                    Err(e) => {
                                        eprintln!("写入文件错误: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            else {
                                println!("=== Gaia 程序 (JSON) ===");
                                println!("{}", json);
                            }
                        }
                        Err(e) => {
                            eprintln!("JSON 序列化错误: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                else {
                    // 输出为可读格式
                    println!("=== Gaia 程序 ===");
                    println!("程序名: {}", gaia_program.name);
                    println!("函数数量: {}", gaia_program.functions.len());
                    println!("常量数量: {}", gaia_program.constants.len());

                    for function in &gaia_program.functions {
                        println!("\n--- 函数: {} ---", function.name);
                        println!("参数: {:?}", function.signature.params);
                        println!("返回类型: {:?}", function.signature.return_type);
                        
                        for block in &function.blocks {
                            println!("  Block: {}", block.label);
                            for (i, instruction) in block.instructions.iter().enumerate() {
                                println!("    {:3}: {:?}", i, instruction);
                            }
                            println!("    Terminator: {:?}", block.terminator);
                        }
                    }

                    if !gaia_program.constants.is_empty() {
                        println!("\n--- 常量 ---");
                        for (name, constant) in &gaia_program.constants {
                            println!("  {}: {:?}", name, constant);
                        }
                    }

                    // 如果指定了输出文件，保存为 JSON
                    if let Some(output_path) = output_file {
                        match serde_json::to_string_pretty(&gaia_program) {
                            Ok(json) => match fs::write(output_path, json) {
                                Ok(_) => println!("\n✅ Gaia 程序已保存为 JSON: {}", output_path),
                                Err(e) => {
                                    eprintln!("写入文件错误: {}", e);
                                    std::process::exit(1);
                                }
                            },
                            Err(e) => {
                                eprintln!("JSON 序列化错误: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Gaia 编译错误: {:?}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // 默认行为：显示解析成功信息
    println!("✅ 解析成功！");
    println!("文件: {}", input_file);
    println!("语句数量: {}", program.statements.len());

    // 统计不同类型的语句
    let mut function_count = 0;
    let mut class_count = 0;
    let mut assignment_count = 0;
    let mut other_count = 0;

    for statement in &program.statements {
        match statement {
            virtual_python::ast::Statement::FunctionDef { .. } => function_count += 1,
            virtual_python::ast::Statement::ClassDef { .. } => class_count += 1,
            virtual_python::ast::Statement::Assignment { .. } => assignment_count += 1,
            _ => other_count += 1,
        }
    }

    println!("  - 函数定义: {}", function_count);
    println!("  - 类定义: {}", class_count);
    println!("  - 赋值语句: {}", assignment_count);
    println!("  - 其他语句: {}", other_count);

    println!("\n💡 提示：");
    println!("  使用 --ast 查看抽象语法树");
    println!("  使用 --tokens 查看词法分析结果");
    println!("  使用 --gaia 编译到 Gaia 指令");
    println!("  使用 --gaia-json 编译到 Gaia 指令 (JSON 格式)");
    println!("  使用 --pyc 编译到 Python 字节码 (.pyc)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_python_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "x = 42").unwrap();
        writeln!(temp_file, "print(x)").unwrap();

        let mut frontend = MiniPythonFrontend::new();
        let source = fs::read_to_string(temp_file.path()).unwrap();
        let result = frontend.parse(&source);

        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn test_compile_to_gaia() {
        let mut frontend = MiniPythonFrontend::new();
        let source = "x = 42\nprint(x)";

        let result = frontend.compile_to_gaia(source);
        assert!(result.is_ok());

        let gaia_program = result.unwrap();
        assert_eq!(gaia_program.name, "python_program");
        assert!(!gaia_program.functions.is_empty());
    }

    #[test]
    fn test_tokenize_simple_code() {
        let mut frontend = MiniPythonFrontend::new();
        let source = "x = 42";

        let result = frontend.tokenize(source);
        assert!(result.is_ok());

        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }
}
