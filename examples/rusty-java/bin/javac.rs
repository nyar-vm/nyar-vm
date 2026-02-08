use rusty_java::MiniJavaFrontend;
use nyar_vm::NyarDriver;
use oak_vfs::vfs::disk::DiskVfs;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: javac <input_file> [-o <output_file>]");
        exit(1);
    }

    let input_file = &args[1];
    let frontend = MiniJavaFrontend::default();
    let driver = NyarDriver::new();
    let vfs = DiskVfs::new();

    // 编译到 JVM .class
    let output_file = args
        .iter()
        .position(|a| a == "-o")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("Hello.class");

    if let Err(e) = driver.compile_to_jvm(&frontend, &vfs, input_file, output_file) {
        eprintln!("Compilation error: {:?}", e);
        exit(1);
    }
}
