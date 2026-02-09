use clap::Parser;
use rusty_go::frontend::RustyGoFrontend;
use rusty_go::runtime::RustyGoRuntime;
use nyar_types::NyarFrontend;
use nyar_vm::vm::core::NyarEnv;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input Go file
    #[arg(short, long)]
    input: PathBuf,

    /// Optimize the code
    #[arg(short, long)]
    optimize: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // 1. Read the input file
    let source = std::fs::read_to_string(&args.input)?;

    // 2. Initialize Frontend and Runtime
    let frontend = RustyGoFrontend::new();
    let mut runtime = RustyGoRuntime::new();

    // 3. Parse and lower to UIR
    println!("Parsing and lowering {}...", args.input.display());
    let ast = frontend.parse(&source).map_err(|e| format!("Parse error: {:?}", e))?;
    
    let mut ctx = nyar_types::NyarContext::default();
    let root_id = frontend.lower_unified(&ast, &mut ctx);
    
    let egraph = ctx.take_egraph();

    // 4. Optimize if requested
    let intent_graph = if args.optimize {
        println!("Optimizing...");
        let optimizer = rusty_go::optimizer::RustyGoOptimizer::new();
        optimizer.optimize((egraph, root_id))
    } else {
        (egraph, root_id)
    };

    // 5. Execute
    println!("Executing...");
    runtime.execute(intent_graph).map_err(|e| format!("Runtime error: {}", e))?;

    Ok(())
}
