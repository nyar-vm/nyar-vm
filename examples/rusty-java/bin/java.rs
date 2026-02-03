use rusty_java::MiniJavaFrontend;
use nyar_vm::NyarDriver;
use oak_vfs::vfs::disk::DiskVfs;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: java <input_file>");
        exit(1);
    }

    let input_file = &args[1];
    let frontend = MiniJavaFrontend::default();
    let mut driver = NyarDriver::new();
    let vfs = DiskVfs::new();

    if let Err(e) = driver.run_source(&frontend, &vfs, input_file) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
    }
}
