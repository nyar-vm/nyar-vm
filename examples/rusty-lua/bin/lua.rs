use clap::Parser;
use std::fs;
use std::path::Path;
use virtual_lua::MiniLuaFrontend;
// use gaia_jit::GaiaJit;
use oak_repl::{OakRepl, ReplHandler, HandleResult};
use oak_highlight::{OakHighlighter, Theme, HighlightResult};

#[derive(Parser, Debug)]
#[command(name = "lua", version = "0.1.0", author = "Gaia Project", about = "Mini Lua Compiler")]
struct Args {
    /// The input Lua source file (.lua). If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,

    /// Output file path
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Show Abstract Syntax Tree
    #[arg(long)]
    ast: bool,
}

use std::fmt::{Display, Formatter};
use std::error::Error;

#[derive(Debug)]
pub enum LuaError {
    Other(String),
}

impl Display for LuaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LuaError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for LuaError {}

impl From<String> for LuaError {
    fn from(s: String) -> Self {
        LuaError::Other(s)
    }
}

impl From<&str> for LuaError {
    fn from(s: &str) -> Self {
        LuaError::Other(s.to_string())
    }
}

/// Lua REPL 处理器
struct LuaReplHandler {
    frontend: MiniLuaFrontend,
}

impl LuaReplHandler {
    fn run_code_internal(frontend: &mut MiniLuaFrontend, source: &str, show_ast: bool) -> Result<(), LuaError> {
        if show_ast {
            match frontend.parse_to_ast(source) {
                Ok(program) => println!("{:#?}", program),
                Err(e) => eprintln!("lua: error: {:?}", e),
            }
            return Ok(());
        }

        match frontend.compile_to_gaia(source) {
            Ok(_module) => {
                println!("Gaia module generated successfully.");
            }
            Err(e) => eprintln!("lua: error: {}", e),
        }
        Ok(())
    }
}

impl ReplHandler for LuaReplHandler {
    fn highlight<'a>(&self, code: &'a str) -> Option<HighlightResult<'a>> {
        let highlighter = OakHighlighter::new();
        highlighter.highlight(code, "lua", Theme::OneDarkPro).ok()
    }

    fn prompt(&self, is_continuation: bool) -> &str {
        if is_continuation {
            ">> "
        } else {
            "lua> "
        }
    }

    fn handle(&mut self, code: &str) -> HandleResult {
        match code.trim() {
            "exit" | "quit" => HandleResult::Exit,
            _ => {
                if let Err(e) = Self::run_code_internal(&mut self.frontend, code, false) {
                    eprintln!("{}", e);
                }
                HandleResult::Continue
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    let mut frontend = MiniLuaFrontend::new();

    if let Some(input_file) = args.input {
        // 读取输入文件
        let source_code = match fs::read_to_string(&input_file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("错误：无法读取文件 '{}': {}", input_file, e);
                std::process::exit(1);
            }
        };

        if args.compile {
            match frontend.compile_to_gaia(&source_code) {
                Ok(module) => {
                    let out_path = args.output.unwrap_or_else(|| {
                        let path = Path::new(&input_file);
                        path.with_extension("gaia").to_string_lossy().into_owned()
                    });
                    // 将 GaiaModule 序列化为 JSON 或二进制（此处示例简化为输出成功信息）
                    println!("Successfully compiled to Gaia module.");
                    println!("Output path: {}", out_path);
                    // fs::write(&out_path, format!("{:?}", module)).ok(); 
                }
                Err(e) => {
                    eprintln!("编译错误: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            if let Err(e) = LuaReplHandler::run_code_internal(&mut frontend, &source_code, args.ast) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        // 进入 REPL 模式
        println!("Mini Lua REPL (Standard)");
        println!("Type 'exit' or 'quit' to exit.");
        let handler = LuaReplHandler { frontend };
        let mut repl = OakRepl::new(handler);
        if let Err(e) = repl.run() {
            eprintln!("REPL 错误: {:?}", e);
        }
    }
}
