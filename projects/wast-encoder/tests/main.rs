use std::str::FromStr;

use wast_encoder::{
    DependentGraph, ExternalFunction, Identifier, ResolveDependencies, VariantItem, VariantType, WasiModule, WasiParameter,
    WasiResource, WasiType,
};

fn define_io_types() -> DependentGraph {
    let mut global = DependentGraph::default();

    let wasi_io_error = WasiModule::from_str("wasi:io/error@0.2.0").unwrap();
    let wasi_io_streams = WasiModule::from_str("wasi:io/streams@0.2.0").unwrap();
    let wasi_io_get = WasiModule::from_str("wasi:io/get@0.2.0").unwrap();

    WasiResource::new(wasi_io_error.clone(), "error", "std::io::IoError").define_language_types(&mut global);
    WasiResource::new(wasi_io_streams.clone(), "output-stream", "std::io::OutputStream").define_language_types(&mut global);
    let mut stream_error = VariantType::new("std::io::StreamError");
    stream_error += VariantItem::new("LastOperationFailed")
        .with_fields(WasiType::TypeHandler { name: Identifier::from_str("std::io::IoError").unwrap(), own: true });
    stream_error += VariantItem::new("Closed");
    stream_error.define_language_types(&mut global);

    let mut f0 = ExternalFunction::new(wasi_io_streams.clone(), "blocking-write", "std::io::OutputStream::write_and_flush");
    f0 += WasiParameter::new(
        "self",
        WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
    );
    f0 += WasiType::Result {
        success: None,
        failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
    };
    f0.define_language_types(&mut global);

    let mut f1 = ExternalFunction::new(
        wasi_io_streams.clone(),
        "blocking-write-and-flush",
        "std::io::OutputStream::blocking_write_and_flush",
    );
    f1 += WasiParameter::new(
        "self",
        WasiType::TypeHandler { name: Identifier::from_str("std::io::OutputStream").unwrap(), own: false },
    );
    f1 += WasiType::Result {
        success: None,
        failure: Some(Box::new(WasiType::TypeAlias { name: Identifier::from_str("std::io::StreamError").unwrap() })),
    };
    f1.define_language_types(&mut global);

    let mut f2 = ExternalFunction::new(wasi_io_get.clone(), "get-std-out", "std::io::standard_output");
    f2.output = Some(WasiType::TypeAlias { name: Identifier::from_str("std::io::OutputStream").unwrap() });
    f2.define_language_types(&mut global);

    global
}

#[test]
fn test_hello_world() {
    let mut global = define_io_types();
    let mut dag = global.resolve_imports().unwrap();
    for import in dag {
        println!("{import:#?}");
    }

    // let graph = global.finalize();
    // for items in graph.topological_sort() {
    //     println!("{items:#?}");
    // }

    // let mut source = CanonicalWasi::default();
    // let wast = source.encode();
    // println!("{wast}");
}
