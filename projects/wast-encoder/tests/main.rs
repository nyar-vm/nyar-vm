use std::{io::Write, path::Path, str::FromStr, sync::Arc};

use wast_encoder::{
    CanonicalWasi, DependentGraph, Identifier, WasiFunction, WasiModule, WasiParameter, WasiRecordField, WasiRecordType,
    WasiResource, WasiType, WasiTypeReference, WasiVariantItem, WasiVariantType,
};

fn define_io_types() -> DependentGraph {
    let mut global = DependentGraph::default();
    let m_debugger = WasiModule::from_str("unstable:debugger/print").unwrap();
    let m_io_error = WasiModule::from_str("wasi:io/error@0.2.0").unwrap();
    let m_io_streams = WasiModule::from_str("wasi:io/streams@0.2.0").unwrap();
    {
        global += WasiResource::new(m_io_error.clone(), "error", "std::io::IoError");
        global += WasiResource::new(m_io_streams.clone(), "output-stream", "std::io::OutputStream");
        global += WasiResource::new(m_io_streams.clone(), "input-stream", "std::io::InputStream");
        let mut stream_error = WasiVariantType::new("std::io::StreamError");
        stream_error += WasiVariantItem::new("LastOperationFailed")
            .with_fields(WasiTypeReference::new(Identifier::from_str("std::io::IoError").unwrap()));
        stream_error += WasiVariantItem::new("Closed");
        global += stream_error;
    }
    {
        let mut point = WasiRecordType::new(Identifier::from_str("Point").unwrap());
        point += WasiRecordField::new(Arc::from("x"), WasiType::Float32);
        point += WasiRecordField::new(Arc::from("y"), WasiType::Float32);
        global += point;
        let mut printer = WasiFunction::external(m_debugger.clone(), "print-point", "test::print_point");
        printer.inputs.push(WasiParameter::new("value", WasiTypeReference::new(Identifier::from_str("Point").unwrap())));
        global += printer;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:cli/stdin@0.2.0").unwrap();
        let mut function = WasiFunction::external(wasi_cli_get.clone(), "get-stdin", "std::io::standard_input");
        function.output = Some(WasiTypeReference::owned(Identifier::from_str("std::io::InputStream").unwrap()).into());
        global += function;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:cli/stdout@0.2.0").unwrap();
        let mut function = WasiFunction::external(wasi_cli_get.clone(), "get-stdout", "std::io::standard_output");
        function.output = Some(WasiTypeReference::owned(Identifier::from_str("std::io::OutputStream").unwrap()).into());
        global += function;
    }
    {
        let wasi_cli_get = WasiModule::from_str("wasi:cli/stderr@0.2.0").unwrap();
        let mut function = WasiFunction::external(wasi_cli_get.clone(), "get-stderr", "std::io::standard_error");
        function.output = Some(WasiTypeReference::owned(Identifier::from_str("std::io::OutputStream").unwrap()).into());
        global += function;
    }
    {
        // let mut f1 = WasiExternalFunction::new(
        //     wasi_io_streams.clone(),
        //     "[method]output-stream.blocking-write-zeroes-and-flush",
        //     "std::io::OutputStream::write_zeros",
        // );
        // f1 += WasiParameter::new(
        //     "self",
        //     WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
        // );
        // f1 += WasiParameter::new("len", WasiType::Integer64 { signed: false });
        // f1 += WasiType::Result {
        //     success: None,
        //     failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
        // };
        // global += f1;
    }
    {
        // let mut f1 = WasiExternalFunction::new(
        //     wasi_io_streams.clone(),
        //     "[method]output-stream.blocking-write-and-flush",
        //     "std::io::OutputStream::write",
        // );
        // f1 += WasiParameter::new(
        //     "self",
        //     WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
        // );
        // f1 += WasiParameter::new("contents", WasiArrayType::new(WasiType::Integer8 { signed: false }));
        // f1 += WasiType::Result {
        //     success: None,
        //     failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
        // };
        // global += f1;
    }
    {
        let mut function = WasiFunction::external(m_debugger.clone(), "print-i32", "print_i32");
        function.inputs.push(WasiParameter::new("value", WasiType::Integer32 { signed: true }));
        global += function;
    }
    {
        let wasi_cli_get = WasiModule::from_str("unstable:debugger/print").unwrap();
        let mut function = WasiFunction::external(wasi_cli_get.clone(), "print-u32", "print_u32");
        function.inputs.push(WasiParameter::new("value", WasiType::Integer32 { signed: true }));
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
