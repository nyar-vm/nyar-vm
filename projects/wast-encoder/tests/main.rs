use std::{io::Write, path::Path, str::FromStr};

use wast_encoder::{
    CanonicalWasi, DependentGraph, ExternalFunction, Identifier, VariantItem, VariantType, WasiModule, WasiParameter,
    WasiResource, WasiType,
};

fn define_io_types() -> DependentGraph {
    let mut global = DependentGraph::default();

    let wasi_io_error = WasiModule::from_str("wasi:io/error@0.2.0").unwrap();
    let wasi_io_streams = WasiModule::from_str("wasi:io/streams@0.2.0").unwrap();

    global += WasiResource::new(wasi_io_error.clone(), "error", "std::io::IoError");
    global += WasiResource::new(wasi_io_streams.clone(), "output-stream", "std::io::OutputStream");
    global += WasiResource::new(wasi_io_streams.clone(), "input-stream", "std::io::InputStream");
    let mut stream_error = VariantType::new("std::io::StreamError");
    stream_error += VariantItem::new("LastOperationFailed")
        .with_fields(WasiType::TypeHandler { name: Identifier::from_str("std::io::IoError").unwrap(), own: true });
    stream_error += VariantItem::new("Closed");
    global += stream_error;

    {
        // let mut f0 = ExternalFunction::new(wasi_io_streams.clone(), "blocking-write", "std::io::OutputStream::write_and_flush");
        // f0 += WasiParameter::new(
        //     "self",
        //     WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
        // );
        // f0 += WasiType::Result {
        //     success: None,
        //     failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
        // };
        // global += f0;
    }

    {
        let mut f1 =
            ExternalFunction::new(wasi_io_streams.clone(), "[method]output-stream.write", "std::io::OutputStream::write");
        f1 += WasiParameter::new(
            "self",
            WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
        );
        f1 += WasiParameter::new("contents", WasiType::Array { inner: Box::new(WasiType::Integer8 { signed: false }) });
        f1 += WasiType::Result {
            success: None,
            failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
        };
        global += f1;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:cli/stdin@0.2.0").unwrap();
        let mut function = ExternalFunction::new(wasi_cli_get.clone(), "get-stdin", "std::io::standard_input");
        function.output =
            Some(WasiType::TypeHandler { name: Identifier::from_str("std::io::InputStream").unwrap(), own: true });
        global += function;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:cli/stdout@0.2.0").unwrap();
        let mut function = ExternalFunction::new(wasi_cli_get.clone(), "get-stdout", "std::io::standard_output");
        function.output =
            Some(WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: true });
        global += function;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:cli/stderr@0.2.0").unwrap();
        let mut function = ExternalFunction::new(wasi_cli_get.clone(), "get-stderr", "std::io::standard_error");
        function.output =
            Some(WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: true });
        global += function;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:debugger/print").unwrap();
        let mut function = ExternalFunction::new(wasi_cli_get.clone(), "print-i8", "print_i8");
        function.inputs.push(WasiParameter::new("i", WasiType::Integer8 { signed: true }));
        global += function;
    }
    global
}

#[test]
fn test_hello_world() {
    let component = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wasm-interpreter/tests/component.wat");
    let mut wat = std::fs::File::create(component).unwrap();

    let source = CanonicalWasi::new(define_io_types()).unwrap();

    println!("{}", source.draw_mermaid());

    let wast = source.encode();
    wat.write_all(wast.as_bytes()).unwrap();
}
