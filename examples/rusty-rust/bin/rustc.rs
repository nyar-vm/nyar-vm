//! Mini Rust 语言编译器
//!
//! 一个简化的 Rust 语言实现，支持编译到多个目标平台

use clap::{Parser, ValueEnum};
use gaia_types::{
    helpers::{AbiCompatible, ApiCompatible, Architecture, CompilationTarget, Url},
    *,
};
use rusty_rust::codegen::MiniRustParser;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "rustc",
    version = "0.1.0",
    author = "Aster <192607617@qq.com>",
    about = "Mini Rust Compiler (Simulating rustc)"
)]
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 读取源文件
    let source = fs::read_to_string(&cli.input).map_err(|e| {
        let url = Url::from_file_path(
            &cli.input
                .canonicalize()
                .unwrap_or_else(|_| cli.input.clone()),
        )
        .unwrap_or_else(|_| Url::parse(&format!("file://{}", cli.input.display())).unwrap());
        GaiaError::io_error(e, url)
    })?;

    if cli.verbose {
        println!("正在解析文件: {:?}", cli.input);
    }

    // 这里可以根据原来的 main.rs 逻辑继续实现具体的编译调用
    // 为了保持 bin 目录的整洁，我们将逻辑委托给 lib.rs 或者在 bin 中保持紧凑
    println!("Simulating rustc compilation for: {:?}", cli.input);

    Ok(())
}
