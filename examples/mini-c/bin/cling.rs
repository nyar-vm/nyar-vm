use clap::Parser;
use std::fs;
use mini_c::frontend::MiniCFrontend;
use mini_c::optimizer::MiniCOptimizer;
use mini_c::runtime::MiniCRuntime;
// use gaia_jit::JitMemory;
use oak_repl::{OakRepl, ReplHandler, HandleResult};
// use oak_highlight::{OakHighlighter, Theme, HighlightResult};

#[derive(Parser, Debug)]
#[command(name = "cling", version = "0.1.0", author = "Gaia Project", about = "Mini C Interpreter (Simulating Cling)")]
struct Args {
    /// The input C file. If not provided, enters REPL mode.
    #[arg(index = 1)]
    input: Option<String>,
}

struct CReplHandler {
    frontend: MiniCFrontend,
}

impl CReplHandler {
    fn run_code_internal(frontend: &mut MiniCFrontend, source: &str) -> anyhow::Result<()> {
        match frontend.parse(source) {
            Ok((egraph, root)) => {
                println!("EGraph nodes: {}", egraph.memo.len());
                println!("Root ID: {:?}", root);
                
                // 1. Optimize
                let mut optimizer = MiniCOptimizer::new();
                let optimized_graph = optimizer.optimize((egraph, root));
                
                // 2. Execute/Compile
                let mut runtime = MiniCRuntime::new();
                if let Err(e) = runtime.execute(optimized_graph) {
                    eprintln!("cling: runtime error: {:?}", e);
                }
            }
            Err(e) => eprintln!("cling: compilation error: {:?}", e),
        }
        Ok(())
    }
}

impl ReplHandler for CReplHandler {
    fn prompt(&self, is_continuation: bool) -> &str {
        if is_continuation { "  ... " } else { "[cling]$ " }
    }

    fn is_complete(&self, code: &str) -> bool {
        if code.trim().is_empty() {
            return true;
        }
        
        let mut depth = 0;
        for c in code.chars() {
            match c {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }
        
        // C 语言通常需要分号结束语句，除非是块定义
        if depth <= 0 {
            let trimmed = code.trim_end();
            if trimmed.ends_with(';') || trimmed.ends_with('}') {
                return true;
            }
        }
        false
    }

    fn handle_line(&mut self, line: &str) -> anyhow::Result<HandleResult> {
        let trimmed = line.trim();
        if trimmed == ".q" || trimmed == "exit()" {
            return Ok(HandleResult::Exit);
        }
        let _ = Self::run_code_internal(&mut self.frontend, line);
        Ok(HandleResult::Continue)
    }

    fn get_indent(&self, code: &str) -> usize {
        let last_line = code.lines().last().unwrap_or("");
        let current_indent = last_line.len() - last_line.trim_start().len();
        if last_line.trim_end().ends_with('{') {
            current_indent + 4
        } else {
            current_indent
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // 假设 mini-c 导出了 MiniCFrontend
    // 注意：如果 mini-c 的库名不是 virtual_c，请根据实际情况调整
    let mut frontend = MiniCFrontend::new();

    if let Some(input_file) = args.input {
        let source_code = fs::read_to_string(&input_file)?;
        CReplHandler::run_code_internal(&mut frontend, &source_code)?;
    } else {
        println!("*******************************************************************************");
        println!("* Visual C++ (Mini-C Cling Simulator)                                         *");
        println!("* Type \".q\" to exit.                                                          *");
        println!("*******************************************************************************");
        
        let handler = CReplHandler { frontend };
        let mut repl = OakRepl::new(handler);
        repl.run()?;
    }
    Ok(())
}
