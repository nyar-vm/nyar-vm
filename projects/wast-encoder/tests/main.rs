use std::{str::FromStr, sync::Arc};

use wast_encoder::{
    CanonicalWasi, DependencyLogger, DependentRegistry, ResolveDependencies, VariantItem, VariantType, WasiFunction,
    WasiModule, WasiParameter, WasiResource, WasiType,
};

#[test]
fn test_hello_world() {
    let mut global = DependentRegistry::default();

    let mut source = CanonicalWasi::default();
    let wasi_io_error = WasiModule::from_str("wasi:io/error@0.2.0").unwrap();
    let wasi_io_streams = WasiModule::from_str("wasi:io/streams@0.2.0").unwrap();

    global += WasiResource::new(wasi_io_error.clone(), "error", "std::io::IoError");
    global += WasiResource::new(wasi_io_streams.clone(), "output-stream", "std::io::OutputStream");
    //     (type $stream-error (variant
    //         (case "last-operation-failed" (own $io-error))
    //         (case "closed")
    //     ))

    let mut stream_error = VariantType::new("std::io::StreamError");
    stream_error +=
        VariantItem::new("LastOperationFailed", WasiType::TypeHandler { name: Arc::from("std::io::IoError"), own: true });
    stream_error += VariantItem::new("Closed", WasiType::TypeAlias { name: Arc::from("std::io::IoError") });

    let mut tracer = DependencyLogger::default();
    let mut f1 = WasiFunction::new(
        wasi_io_streams.clone(),
        "blocking-write-and-flush",
        "std::io::OutputStream::blocking_write_and_flush",
    );
    f1 += WasiParameter::new("self", WasiType::TypeHandler { name: Arc::from("std::io::OutputStream"), own: false });
    f1 += WasiType::Result {
        success: None,
        failure: Some(Box::new(WasiType::TypeAlias { name: Arc::from("std::io::IoError") })),
    };
    f1.trace_language_types(&mut tracer);
    println!("{tracer:#?}");
    global += f1;

    let wast = source.encode();
    println!("{wast}");
}
