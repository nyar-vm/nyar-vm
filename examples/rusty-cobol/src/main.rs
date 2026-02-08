use mini_cobol::MiniCobolFrontend;
use nyar_vm::NyarDriver;
use std::{path::Path, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cobol <input_file>");
        exit(1);
    }

    let input_file = Path::new(&args[1]);
    let frontend = MiniCobolFrontend::new();
    let driver = NyarDriver::new();
    
    if let Err(e) = driver.run_source(&frontend, input_file) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
