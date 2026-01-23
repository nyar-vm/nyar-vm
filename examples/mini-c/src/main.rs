use clap::{Arg, Command};
use mini_c::{frontend::MiniCFrontend, optimizer::MiniCOptimizer, runtime::MiniCRuntime};
use std::fs;

fn main() {
    let matches = Command::new("Mini C Interpreter")
        .version("0.1.0")
        .author("Nyar Project")
        .about("A simple C interpreter using Oaks, Chomsky, and Gaia")
        .arg(Arg::new("input").help("Input C file").required(true).index(1))
        .arg(Arg::new("ast").long("ast").help("Output AST").action(clap::ArgAction::SetTrue))
        .get_matches();

    let input_file = matches.get_one::<String>("input").unwrap();
    let source = fs::read_to_string(input_file).expect("Failed to read input file");

    let frontend = MiniCFrontend::new();
    let optimizer = MiniCOptimizer::new();
    let mut runtime = MiniCRuntime::new();

    // 1. Frontend: Parse to UAST
    match frontend.parse(&source) {
        Ok(uast) => {
            if matches.get_flag("ast") {
                println!("{:#?}", uast);
                return;
            }

            // 2. Optimizer: Optimize UAST
            let optimized_uast = optimizer.optimize(uast);

            // 3. Runtime: Execute
            if let Err(e) = runtime.execute(optimized_uast) {
                eprintln!("Runtime error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
