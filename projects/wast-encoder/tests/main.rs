use std::sync::Arc;

use wast_encoder::{CanonicalWasi, WasiFunction, WasiInstance, WasiParameter, WasiResource, WasiType};

#[test]
fn test() {
    let mut source = CanonicalWasi::default();
    let mut io = WasiInstance::new("wasi:io/error@0.2.0");
    io += WasiResource::new("std::io::IoError", "error");
    io += WasiResource::new("std::io::StreamError", "stream-error");
    io += WasiResource::new("std::io::RuntimeError", "runtime-error");
    //         (export "[method]output-stream.blocking-write-and-flush"
    //             (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result (result (error $stream-error))))
    //         )
    let mut f1 =
        WasiFunction::method("std::io::OutputStream::blocking-write-and-flush", "output-stream", "blocking-write-and-flush");
    f1 += WasiParameter::new("self", WasiType::TypeHandler { name: Arc::from("std::io::OutputStream"), own: false });
    f1 += WasiType::Result {
        success: None,
        failure: Some(Box::new(WasiType::TypeAlias { name: Arc::from("std::io::StreamError") })),
    };
    io += f1;

    source += io;

    let wast = source.encode();
    println!("{wast}");
}
