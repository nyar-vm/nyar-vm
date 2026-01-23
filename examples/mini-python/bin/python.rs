use clap::Parser;
use std::fs;
use std::path::Path;
use virtual_python::MiniPythonFrontend;
use gaia_jit::GaiaJit;
use oak_repl::{OakRepl, ReplHandler};

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
            match frontend.parse(source) {
                Ok(program) => println!("{:#?}", program),
                Err(e) => eprintln!("python: error: {:?}", e),
            }
            return Ok(());
        }

        match frontend.compile_to_gaia(source) {
            Ok(module) => {
                let mut jit = GaiaJit::new();
                if let Err(e) = jit.load_module(module) {
                    eprintln!("python: JIT load error: {:?}", e);
                    return Ok(());
                }
                if let Err(e) = jit.run("main") {
                    eprintln!("python: execution error: {:?}", e);
                }
            }
            Err(e) => eprintln!("python: error: {}", e),
        }
        Ok(())
    }
}

impl ReplHandler for PythonReplHandler {
    fn handle_line(&mut self, line: &str) -> anyhow::Result<bool> {
        if line == "exit()" || line == "quit()" {
            return Ok(false);
        }
        let _ = Self::run_code_internal(&mut self.frontend, line, false);
        Ok(true)
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
        let mut repl = OakRepl::new(">>> ", handler);
        repl.run()?;
    }
    Ok(())
}
