use clap::Parser;
use std::fs;
use std::path::Path;
use virtual_python::MiniPythonFrontend;
// use gaia_jit::GaiaJit;
use oak_repl::{OakRepl, ReplHandler, HandleResult};
use oak_highlight::{OakHighlighter, Theme, HighlightResult};

#[derive(Parser, Debug)]
#[command(name = "python", version = "0.1.0", author = "Gaia Project", about = "Mini Python Interpreter (Standard)")]
struct Args {
    /// The input Python source file (.py). If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,

    /// Compile to Python bytecode (.pyc) instead of executing
    #[arg(long)]
    compile: bool,

    /// Output file path
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Show Abstract Syntax Tree
    #[arg(long)]
    ast: bool,
}

/// Python REPL 处理器
struct PythonReplHandler {
    frontend: MiniPythonFrontend,
}

impl PythonReplHandler {
    fn run_code_internal(frontend: &mut MiniPythonFrontend, source: &str, show_ast: bool) -> anyhow::Result<()> {
        if show_ast {
            match frontend.parse_to_ast(source) {
                Ok(program) => println!("{:#?}", program),
                Err(e) => eprintln!("python: error: {:?}", e),
            }
            return Ok(());
        }

        match frontend.compile_to_gaia(source) {
            Ok(_module) => {
                println!("Gaia module generated successfully. JIT execution is not yet implemented in this standard driver.");
                /*
                let mut jit = GaiaJit::new();
                if let Err(e) = jit.load_module(module) {
                    eprintln!("python: JIT load error: {:?}", e);
                    return Ok(());
                }
                if let Err(e) = jit.run("main") {
                    eprintln!("python: execution error: {:?}", e);
                }
                */
            }
            Err(e) => eprintln!("python: error: {}", e),
        }
        Ok(())
    }
}

impl ReplHandler for PythonReplHandler {
    fn highlight<'a>(&self, code: &'a str) -> Option<HighlightResult<'a>> {
        let highlighter = OakHighlighter::new();
        highlighter.highlight(code, "python", Theme::OneDarkPro).ok()
    }

    fn prompt(&self, is_continuation: bool) -> &str {
        if is_continuation { "... " } else { ">>> " }
    }

    fn is_complete(&self, code: &str) -> bool {
        if code.trim().is_empty() {
            return true;
        }
        
        // 简单的完整性检查：括号匹配
        let mut depth = 0;
        for c in code.chars() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            }
        }
        
        if depth > 0 {
            return false;
        }

        // 如果以冒号结尾，说明需要下一行
        if code.trim_end().ends_with(':') {
            return false;
        }
        
        // 如果最后一行不为空，且代码中包含冒号（可能在 if/def 块中），
        // 且当前代码没有以空行结尾，通常 Python REPL 需要一个额外空行来结束块
        if code.contains(':') && !code.ends_with("\n\n") && !code.ends_with("\n") {
             // 这里逻辑可以根据具体前端解析能力调整
        }

        true
    }

    fn handle_line(&mut self, line: &str) -> anyhow::Result<HandleResult> {
        let trimmed = line.trim();
        if trimmed == "exit()" || trimmed == "quit()" {
            return Ok(HandleResult::Exit);
        }
        let _ = Self::run_code_internal(&mut self.frontend, line, false);
        Ok(HandleResult::Continue)
    }

    fn get_indent(&self, code: &str) -> usize {
        // 简单的自动缩进：如果上一行以冒号结尾，增加 4 个空格
        if code.trim_end().ends_with(':') {
            let last_line = code.lines().last().unwrap_or("");
            let current_indent = last_line.len() - last_line.trim_start().len();
            return current_indent + 4;
        }
        
        // 否则保持当前缩进
        let last_line = code.lines().last().unwrap_or("");
        last_line.len() - last_line.trim_start().len()
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut frontend = MiniPythonFrontend::new();

    if let Some(input_file) = args.input {
        let source_code = match fs::read_to_string(&input_file) {
            Ok(content) => content,
            Err(_) => {
                eprintln!("python: can't open file '{}': [Errno 2] No such file or directory", input_file);
                std::process::exit(1);
            }
        };

        if args.compile {
            match frontend.compile_to_pyc(&source_code, &input_file) {
                Ok(pyc_data) => {
                    let out_path = args.output.unwrap_or_else(|| {
                        let path = Path::new(&input_file);
                        path.with_extension("pyc").to_string_lossy().into_owned()
                    });
                    fs::write(&out_path, pyc_data)?;
                    println!("Successfully compiled to {}", out_path);
                }
                Err(e) => {
                    eprintln!("python: error: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            PythonReplHandler::run_code_internal(&mut frontend, &source_code, args.ast)?;
        }
    } else {
        println!("Python 0.1.0 (Mini Python, Gaia Project)");
        println!("Type \"help\", \"copyright\", \"credits\" or \"license\" for more information.");
        
        let handler = PythonReplHandler { frontend };
        let mut repl = OakRepl::new(handler);
        repl.run()?;
    }
    Ok(())
}
