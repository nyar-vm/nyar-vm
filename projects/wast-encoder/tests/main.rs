use std::sync::Arc;

use wast_encoder::{CanonicalWasi, DependentRegistry, WasiFunction, WasiInstance, WasiParameter, WasiResource, WasiType};

#[test]
fn test_hello_world() {
    let mut global = DependentRegistry::default();

    let mut source = CanonicalWasi::default();
    let mut io = WasiInstance::new("wasi:io/error@0.2.0");
    global += WasiResource::new("std::io::IoError", "error").with_borrow();
    source += io;

    let mut stream = WasiInstance::new("wasi:io/streams@0.2.0");
    let mut f1 =
        WasiFunction::method("std::io::OutputStream::blocking_write_and_flush", "output-stream", "blocking-write-and-flush");
    f1 += WasiParameter::new("self", WasiType::TypeHandler { name: Arc::from("std::io::OutputStream"), own: false });
    f1 += WasiType::Result {
        success: None,
        failure: Some(Box::new(WasiType::TypeAlias { name: Arc::from("std::io::IoError") })),
    };
    stream += f1;
    source += stream;

    let wast = source.encode();
    println!("{wast}");
}
