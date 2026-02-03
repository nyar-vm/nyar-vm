use rusty_swift::RustySwiftFrontend;
use nyar_vm::NyarDriver;
use oak_vfs::DiskVfs;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: swift <input_file>");
        exit(1);
    }

    let input_file = &args[1];
    let vfs = DiskVfs::new();
    let frontend = RustySwiftFrontend::new();
    let driver = NyarDriver::new();
    
    if let Err(e) = driver.run_source(&frontend, &vfs, input_file) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
