use clap::Parser;
use std::fs;
use nyar_vm::NyarVM;
use nyar_vm::bytecode::format::NyarModule;

#[derive(Parser, Debug)]
#[command(name = "kotlin", version = "0.1.0", author = "Nyar Project", about = "Mini Kotlin Runner")]
struct Args {
    /// The input Nyar Binary file
    #[arg(index = 1)]
    input: String,
}

fn main() {
    let args = Args::parse();

    // Read input file
    let data = match fs::read(&args.input) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Could not read file '{}': {}", args.input, e);
            std::process::exit(1);
        }
    };

    // Decode module
    let module = match NyarModule::parse(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Could not decode Nyar module: {:?}", e);
            std::process::exit(1);
        }
    };

    // Initialize VM
    let mut vm = NyarVM::new();
    
    // Load and run
    let module_idx = vm.load_module(module);
    
    // Find main method to run
    // In Mini Kotlin, we usually look for HelloWorldKt.main or similar
    // For now, let's look for any exported method named "main"
    let mut main_chunk = None;
    for (i, m) in vm.modules.iter().enumerate() {
        for export in &m.exports {
            println!("Found export: symbol={}, chunk_idx={}", export.symbol, export.chunk_idx);
            if export.symbol == "main" {
                main_chunk = Some((i, export.chunk_idx as usize));
                break;
            }
        }
    }

    if let Some((m_idx, c_idx)) = main_chunk {
        if let Err(e) = vm.execute(m_idx, c_idx) {
            vm.print_traceback(&e);
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: Could not find 'main' method");
        std::process::exit(1);
    }
}
