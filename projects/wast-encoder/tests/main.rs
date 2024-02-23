use vcc_runner::{WasiCanonical, WasiInstance};

#[test]
fn test() {
    let mut source = WasiCanonical::default();
    let mut io = WasiInstance::new("wasi:io/error@0.2.0");
    io.add_resource("std::io::IoError", "error");
    io.add_resource("std::io::StreamError", "stream-error");
    io.add_resource("std::io::RuntimeError", "runtime-error");
    source += io;

    let wast = source.encode();
    println!("{wast}");
}
