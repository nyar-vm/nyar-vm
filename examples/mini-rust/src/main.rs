//! Mini Rust 语言编译器
//!
//! 一个简化的 Rust 语言实现，支持编译到多个目标平台

use clap::{Parser, ValueEnum};
use gaia_assembler::{assembler::GaiaAssembler, program::GaiaModule};
use gaia_types::{
    helpers::{AbiCompatible, ApiCompatible, Architecture, CompilationTarget, Url},
    *,
};
use std::{fs, path::PathBuf};

mod ast;
mod codegen;
mod lexer;
mod parser;

use crate::codegen::MiniRustParser;

#[derive(Parser)]
#[command(name = "mini-rust")]
#[command(about = "Mini Rust 语言编译器")]
struct Cli {
    /// 输入的 mini-rust 源文件
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// 目标平台
    #[arg(short, long, value_enum, default_value_t = Target::All)]
    target: Target,

    /// 输出目录
    #[arg(short, long, default_value = "target")]
    output: PathBuf,

    /// 是否显示详细信息
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Clone, ValueEnum)]
enum Target {
    /// 编译到所有平台
    All,
    /// .NET IL
    Il,
    /// .NET CLR Binary (DLL)
    Clr,
    /// Java Virtual Machine
    Jvm,
    /// Portable Executable
    Pe,
    /// WebAssembly
    Wasi,
}

fn compilation_target_for(target: Target) -> Option<CompilationTarget> {
    match target {
        Target::All => None,
        Target::Il => Some(CompilationTarget {
            build: Architecture::CLR,
            host: AbiCompatible::MicrosoftIntermediateLanguage,
            target: ApiCompatible::ClrRuntime(4),
        }),
        Target::Clr => Some(CompilationTarget {
            build: Architecture::CLR,
            host: AbiCompatible::Unknown,
            target: ApiCompatible::ClrRuntime(4),
        }),
        Target::Jvm => Some(CompilationTarget {
            build: Architecture::JVM,
            host: AbiCompatible::JavaAssembly,
            target: ApiCompatible::JvmRuntime(8),
        }),
        Target::Pe => Some(CompilationTarget {
            build: Architecture::X86_64,
            host: AbiCompatible::PE,
            target: ApiCompatible::MicrosoftVisualC,
        }),
        Target::Wasi => Some(CompilationTarget {
            build: Architecture::WASM32,
            host: AbiCompatible::WebAssemblyTextFormat,
            target: ApiCompatible::WASI,
        }),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 读取源文件
    let source = fs::read_to_string(&cli.input).map_err(|e| {
        let url = Url::from_file_path(&cli.input.canonicalize().unwrap_or_else(|_| cli.input.clone()))
            .unwrap_or_else(|_| Url::parse(&format!("file://{}", cli.input.display())).unwrap());
        GaiaError::io_error(e, url)
    })?;

    if cli.verbose {
        println!("正在解析文件: {:?}", cli.input);
    }

    // 解析 mini-rust 源代码
    let program = MiniRustParser::parse(&source)?;

    if cli.verbose {
        println!("解析完成，生成的程序:");
        println!("{:#?}", program);
    }

    // 创建输出目录
    fs::create_dir_all(&cli.output).map_err(|e| GaiaError::io_error(e, Url::from_file_path(&cli.output).unwrap()))?;

    // 编译到目标平台
    match compilation_target_for(cli.target) {
        Some(target_platform) => {
            compile_to_single_platform(&program, target_platform, &cli.output, cli.verbose)?;
        }
        None => {
            compile_to_all_platforms(&program, &cli.output, cli.verbose)?;
        }
    }

    println!("编译完成！");
    Ok(())
}

fn compile_to_single_platform(
    program: &GaiaModule,
    target: CompilationTarget,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("正在编译到 {} 平台...", target);
    }

    let assembler = GaiaAssembler::new();
    let generated = assembler.compile(program, &target)?;

    // 写出已生成的文件
    let mut wrote_any = false;
    for (filename, bytes) in generated.files {
        let output_file = output_dir.join(filename);
        fs::write(&output_file, bytes).map_err(|e| GaiaError::io_error(e, Url::from_file_path(&output_file).unwrap()))?;
        println!("已生成 {} 文件: {:?}", target, output_file);
        wrote_any = true;
    }

    // 针对 PE 目标的回退：如果未生成任何文件，则直接调用 PE 后端生成
    if !wrote_any && matches!(target.host, AbiCompatible::PE) {
        if verbose {
            println!("未检测到后端输出，启用 PE 回退生成...");
        }
        let pe_bytes = gaia_assembler::backends::pe::compile(program)?;
        let output_file = output_dir.join("main.exe");
        fs::write(&output_file, pe_bytes).map_err(|e| GaiaError::io_error(e, Url::from_file_path(&output_file).unwrap()))?;
        println!("已生成 {} 文件: {:?}", target, output_file);
    }
    Ok(())
}

fn compile_to_all_platforms(program: &GaiaModule, output_dir: &PathBuf, verbose: bool) -> Result<()> {
    if verbose {
        println!("正在编译到主选平台...");
    }

    // 暂时跳过 WASI（后端尚未实现函数起始编译），避免影响其它平台输出
    let targets = vec![
        CompilationTarget {
            build: Architecture::CLR,
            host: AbiCompatible::MicrosoftIntermediateLanguage,
            target: ApiCompatible::ClrRuntime(4),
        },
        CompilationTarget { build: Architecture::JVM, host: AbiCompatible::JavaAssembly, target: ApiCompatible::JvmRuntime(8) },
        CompilationTarget { build: Architecture::X86_64, host: AbiCompatible::PE, target: ApiCompatible::MicrosoftVisualC },
    ];

    let assembler = GaiaAssembler::new();
    for target in targets {
        let generated = assembler.compile(program, &target)?;
        for (filename, bytes) in generated.files {
            let output_file = output_dir.join(filename);
            fs::write(&output_file, bytes).map_err(|e| GaiaError::io_error(e, Url::from_file_path(&output_file).unwrap()))?;
            println!("已生成 {} 文件: {:?}", target, output_file);
        }
    }

    Ok(())
}
