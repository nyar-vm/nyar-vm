use rusty_tcl::RustyTclFrontend;
use nyar_vm::NyarDriver;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: tcl <input_file>");
        exit(1);
    }

    let input_file = &args[1];
    let frontend = RustyTclFrontend::new();
    let driver = NyarDriver::new();
    let vfs = driver.default_vfs();

    if let Err(e) = driver.run_source(&frontend, &vfs, input_file) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
