use rusty_julia::RustyJuliaFrontend;
use nyar_vm::NyarDriver;
use std::process::exit;
use oak_vfs::DiskVfs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: julia <input_file>");
        exit(1);
    }

    let input_file = args[1].as_str();
    let frontend = RustyJuliaFrontend::new();
    let driver = NyarDriver::new();
    let vfs = DiskVfs::new();
    
    if let Err(e) = driver.run_source(&frontend, &vfs, input_file) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
