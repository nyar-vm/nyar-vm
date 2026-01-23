use clap::Parser;
use std::fs;
use nyar_vm::runtime::VirtualMachine;
use nyar_vm::bytecode::format::NyarModule;

#[derive(Parser, Debug)]
#[command(name = "java", version = "0.1.0", author = "Nyar Project", about = "Mini Java Runner")]
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
    let module = match NyarModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Could not decode Nyar module: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize VM
    let mut vm = VirtualMachine::new();
    
    // Load and run
    if let Err(e) = vm.load_module(module) {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = vm.run() {
        eprintln!("Execution error: {}", e);
        std::process::exit(1);
    }
}
